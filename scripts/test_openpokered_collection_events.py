"""Registration telemetry must never promote a planned goal to a fact."""
from copy import deepcopy
import io
import json
from pathlib import Path
import sys
import time
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.collection_events import RegistrationEvidence, native_capture_registration
from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.story_agent import DualStoryAgent


def mon(species, level=30, hp=70):
    return {'species': species, 'level': level, 'hp': hp, 'max_hp': hp,
            'status': 'None', 'moves': ['Headbutt', 'None', 'None', 'None'], 'pp': [15, 0, 0, 0]}


def state(party, stored=(), owned=(), frame=10, **extra):
    return {'screen': 'overworld', 'map_name': 'PokemonMansion1F', 'frame_count': frame,
            'party': deepcopy(list(party)), 'stored_pokemon': deepcopy(list(stored)),
            'pokedex': {'owned_species': list(owned), 'owned': len(owned), 'seen': len(owned)}, **extra}


def capture_fixture():
    enemy = {'capture_species': 'Muk', 'species': 'Muk', 'level': 39, 'hp': 135, 'max_hp': 135}
    live = {'is_wild': True, 'is_ghost': False, 'is_safari': False,
            'capture_blocked_reason': None, 'enemy': enemy}
    before = state([mon('Horsea', 31)], owned=['Horsea'], screen='battle', battle_live=live,
        battle_inventory=[{'item': 'UltraBall', 'qty': 3}])
    caught = {**mon('Muk', 39, 42), 'max_hp': 135, 'box': 3, 'index': 0}
    after = state(before['party'], [caught], ['Horsea', 'Muk'], 20, screen='pokedex',
        battle_live={**deepcopy(live), 'enemy': {**enemy, 'hp': 42}}, battle_inventory=[])
    return before, after


class CollectionEventsTests(unittest.TestCase):
    def test_native_capture_matches_owned_bit_roster_enemy_and_real_cost(self):
        before, after = capture_fixture()
        original = deepcopy((before, after))
        witness = native_capture_registration(before, after)['Muk']
        self.assertEqual(witness['method'], 'wild_capture')
        self.assertEqual(witness['ball_costs'], {'ULTRABALL': 3})
        self.assertEqual(witness['caught']['level'], 39)
        self.assertEqual((before, after), original)
        self.assertIn('not original-route legality', witness['scope'])

    def test_no_ball_cost_cannot_claim_capture(self):
        before, after = capture_fixture()
        after['battle_inventory'] = deepcopy(before['battle_inventory'])
        self.assertEqual(native_capture_registration(before, after), {})

    def test_capture_rejects_ghost_trainer_blocked_or_unknown_battle(self):
        for field, value in (('is_ghost', True), ('is_wild', False), ('is_wild', None),
                             ('capture_blocked_reason', 'restless_soul')):
            with self.subTest(field=field, value=value):
                before, after = capture_fixture()
                before['battle_live'][field] = value
                self.assertEqual(native_capture_registration(before, after), {})

    def test_capture_rejects_fainted_or_mismatched_enemy(self):
        for field, value in (('capture_species', 'Grimer'), ('level', 38), ('hp', 0)):
            with self.subTest(field=field):
                before, after = capture_fixture()
                after['battle_live']['enemy'][field] = value
                self.assertEqual(native_capture_registration(before, after), {})

    def test_owned_bit_alone_or_duplicate_roster_is_not_capture_evidence(self):
        for change in ('missing', 'duplicate', 'wrong_level', 'wrong_hp', 'unrelated_removed'):
            with self.subTest(change=change):
                before, after = capture_fixture()
                if change == 'missing':
                    after['stored_pokemon'] = []
                elif change == 'duplicate':
                    after['stored_pokemon'] *= 2
                elif change == 'wrong_level':
                    after['stored_pokemon'][0]['level'] = 38
                elif change == 'wrong_hp':
                    after['stored_pokemon'][0]['hp'] = 41
                else:
                    after['party'] = []
                self.assertEqual(native_capture_registration(before, after), {})

    def test_capture_never_asserts_grass_water_rod_or_static_mode(self):
        before, after = capture_fixture()
        for mode in ('Walking', 'Surfing'):
            before['player_transport'] = mode
            self.assertEqual(native_capture_registration(before, after)['Muk']['method'], 'wild_capture')

    def test_safari_requires_native_marker_and_ball_counter_cost(self):
        before, after = capture_fixture()
        for snapshot, balls in ((before, 30), (after, 28)):
            snapshot['battle_live'].update(is_safari=True, safari={'balls': balls})
        witness = native_capture_registration(before, after)['Muk']
        self.assertEqual(witness['method'], 'safari')
        self.assertEqual(witness['ball_costs'], {'SAFARIBALL': 2})
        after['battle_live']['safari']['balls'] = 30
        self.assertEqual(native_capture_registration(before, after), {})

    def test_tracker_keeps_actual_capture_during_evolution_goal_without_goal_input(self):
        before, after = capture_fixture()
        tracker = RegistrationEvidence()
        self.assertEqual(tracker.registrations(before, {}), {})
        tracker.observe_battle('battle_started', before)
        tracker.observe_battle('battle_resolved', after)
        evidence = tracker.registrations(after, {'Muk': [{'method': 'evolution', 'from_species': 'Grimer'}]})
        self.assertEqual(evidence['Muk']['method'], 'wild_capture')
        self.assertEqual(tracker.registrations(after, {}), {})

    def test_initial_continue_snapshot_is_a_baseline_not_an_acquisition(self):
        tracker = RegistrationEvidence()
        before, after = capture_fixture()
        tracker.observe_battle('battle_started', before)
        tracker.observe_battle('battle_resolved', after)
        self.assertEqual(tracker.registrations(after, {}), {})

    def test_unwitnessed_gift_or_trade_is_unknown_even_if_graph_can_explain_it(self):
        tracker = RegistrationEvidence()
        before = state([mon('Spearow')], owned=['Spearow'])
        after = state([mon('Farfetchd')], owned=['Spearow', 'Farfetchd'])
        tracker.registrations(before, {})
        evidence = tracker.registrations(after, {'Farfetchd': [{'method': 'npc_trade', 'from_species': 'Spearow'}]})
        self.assertEqual(evidence['Farfetchd']['method'], 'unknown')

    def test_native_level_evolution_survives_pc_transfer_and_party_reorder(self):
        tracker = RegistrationEvidence()
        before = state([mon('Seel', 33), mon('Charizard', 64)], owned=['Seel', 'Charizard'])
        phase = state([mon('Seel', 34), mon('Charizard', 64)], owned=['Seel', 'Charizard'],
                      evolution_phase='IsEvolving')
        after = state([mon('Charizard', 64)], [mon('Dewgong', 34)], ['Seel', 'Charizard', 'Dewgong'])
        graph = {'Dewgong': [{'method': 'evolution', 'trigger': 'level', 'from_species': 'Seel', 'level': 34}]}
        tracker.registrations(before, graph)
        tracker.observe_battle('battle_started', before)
        tracker.observe_battle('battle_resolved', phase)
        witness = tracker.registrations(after, graph)['Dewgong']
        self.assertEqual(witness['method'], 'evolution')
        self.assertEqual(witness['before']['level'], witness['after']['level'])

    def test_level_replacement_without_native_phase_does_not_prove_evolution(self):
        tracker = RegistrationEvidence()
        before = state([mon('Seel', 33)], owned=['Seel'])
        after = state([mon('Dewgong', 34)], owned=['Seel', 'Dewgong'])
        graph = {'Dewgong': [{'method': 'evolution', 'trigger': 'level', 'from_species': 'Seel', 'level': 34}]}
        tracker.registrations(before, graph)
        self.assertEqual(tracker.registrations(after, graph)['Dewgong']['method'], 'unknown')

    def test_level_evidence_rejects_wrong_threshold_and_ambiguous_source(self):
        for duplicate in (False, True):
            tracker = RegistrationEvidence()
            mons = [mon('Seel', 33)] * (2 if duplicate else 1)
            before = state(mons, owned=['Seel'])
            after = state([mon('Dewgong', 33)] + mons[1:], owned=['Seel', 'Dewgong'])
            graph = {'Dewgong': [{'method': 'evolution', 'trigger': 'level', 'from_species': 'Seel', 'level': 34}]}
            tracker.registrations(before, graph)
            tracker.observe_battle('battle_resolved', {**before, 'evolution_phase': 'IsEvolving'})
            self.assertEqual(tracker.registrations(after, graph)['Dewgong']['method'], 'unknown')

    def test_stone_evolution_requires_same_level_and_exact_actual_item_cost(self):
        for cost in (0, 1, 2):
            tracker = RegistrationEvidence()
            before = state([mon('Pikachu', 20)], owned=['Pikachu'], bag={'THUNDERSTONE': 2})
            after = state([mon('Raichu', 20)], owned=['Pikachu', 'Raichu'], bag={'THUNDERSTONE': 2 - cost})
            graph = {'Raichu': [{'method': 'evolution', 'trigger': 'item', 'from_species': 'Pikachu', 'item': 'ThunderStone'}]}
            tracker.registrations(before, graph)
            self.assertEqual(tracker.registrations(after, graph)['Raichu']['method'], 'evolution' if cost == 1 else 'unknown')

    def test_invalid_observation_discards_pending_witness(self):
        before, after = capture_fixture()
        tracker = RegistrationEvidence()
        tracker.registrations(before, {})
        tracker.observe_battle('battle_started', before)
        tracker.observe_battle('battle_resolved', after)
        self.assertEqual(tracker.registrations({'party': []}, {}), {})
        self.assertEqual(tracker.registrations(after, {}), {})

    def test_native_record_hook_preserves_original_event(self):
        before, after = capture_fixture()
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex, agent.start_time, agent.trace = True, time.monotonic(), io.StringIO()
        agent.record('battle_started', state=before)
        agent.record('battle_resolved', state=after)
        events = [json.loads(line) for line in agent.trace.getvalue().splitlines()]
        self.assertEqual(events[0]['state'], before)
        self.assertEqual(events[1]['state'], after)
        self.assertEqual(agent._registration_evidence.pending['Muk']['method'], 'wild_capture')

    def test_facts_records_actual_method_and_preserves_planned_method_separately(self):
        before, after = capture_fixture()
        after.update(screen='overworld', box_counts=[0] * 12)
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.client, agent.game, agent.record = Mock(), Mock(), Mock()
        agent.client.state.return_value = after
        agent.client.party.return_value = []
        agent.game.st.return_value = after
        agent.observe_audit_evolution, agent.require_static_sources = Mock(), Mock()
        agent.collection_audit_pending, agent.cleared_terrain = {}, set()
        agent.battle_defeats, agent.visited, agent.crossed_passages = [], set(), set()
        agent.collects_dex, agent.index = True, None
        agent._recorded_dex_species = ('Horsea',)
        agent.active = {'target': ('register', 'Seadra', True), 'context': {'acquisition_method': 'evolution'}}
        agent.complete_collection_graph = Mock(return_value={})
        tracker = agent._registration_evidence = RegistrationEvidence()
        tracker.registrations(before, {})
        tracker.observe_battle('battle_started', before)
        tracker.observe_battle('battle_resolved', after)
        base = {'map': after['map_name'], 'bag': {}, 'flags': {}, 'dex': after['pokedex']}
        with patch.object(DualStoryAgent, 'facts', return_value=base):
            agent.facts()
        event = agent.record.call_args.kwargs
        self.assertEqual(event['acquired'], ['Muk'])
        self.assertEqual(event['acquisition_method'], 'wild_capture')
        self.assertEqual(event['planned_acquisition_method'], 'evolution')
        self.assertEqual(event['acquisition_evidence']['Muk']['caught']['level'], 39)
        self.assertEqual(event['registration_scope'], 'new_registration')


if __name__ == '__main__':
    unittest.main()
