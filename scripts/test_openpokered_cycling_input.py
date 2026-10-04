"""Native-input mechanics, not fixed choices or synthetic collection credit."""
import time
import unittest
from copy import deepcopy
from unittest.mock import Mock

from openpokered.client import AgentClient
from openpokered.playthrough_judgments import JevGame, ObservedProtocol
from openpokered.story_agent import DualStoryAgent, StoryStopped


def road_state(**changes):
    return {'frame_count': 100, 'map_name': 'Route17', 'screen': 'overworld',
        'player_x': 15, 'player_y': 20, 'player_transport': 'Biking',
        'player_movement_state': 'Idle',
        'warp_fade': 'Idle', 'script_running': False, 'script_awaiting_battle': False,
        'fishing_active': False, 'door_exit_pending': False, 'dialogue_state': None,
        'field_menu': None, 'choice': None, 'active_script_effect': None,
        'evolution_phase': None, 'npc_trade_phase': None, **changes}


class NativeFrames:
    def __init__(self, state=None, transition=None):
        self.state = road_state() if state is None else deepcopy(state)
        self.transition = transition
        self.requests, self.buttons = [], []
        self.previous = None

    def cmd(self, **request):
        self.requests.append(deepcopy(request))
        if request['cmd'] == 'get_state':
            return {'ok': True, 'data': deepcopy(self.state)}
        assert request['cmd'] in ('press_timeline', 'step_frames'), request
        start = self.state['frame_count']
        buttons = request.get('buttons', [None] * request.get('count', 0))
        for button in buttons:
            self.buttons.append(button)
            edge = button != self.previous
            if self.transition:
                self.transition(self.state, button, edge)
            self.previous = button
            self.state['frame_count'] += 1
        return {'ok': True, 'data': {'advanced': True, 'queue_start_frame': start,
            'frame_count': self.state['frame_count'], 'stepped': len(buttons)}}


class CyclingInputTests(unittest.TestCase):
    def protocol(self, native=None):
        native = native or NativeFrames()
        rows = []
        protocol = ObservedProtocol(native, lambda kind, **data: rows.append((kind, data)),
            time.monotonic() + 60, brake_cycling_road=True)
        return protocol, native, rows

    def test_wait_is_exact_ordinary_b_frames_with_truthful_receipts(self):
        protocol, native, rows = self.protocol()
        reply = protocol.step(13)
        self.assertEqual(native.buttons, ['b'] * 13)
        self.assertEqual(reply['data']['stepped'], 13)
        self.assertEqual(reply['data']['frame_count'], 113)
        self.assertIs(reply['data']['atomic'], False)
        self.assertEqual(reply['data']['execution'], 'phase_checked_native_chunks')
        inputs = [data for kind, data in rows if kind == 'native_input']
        self.assertEqual(len(inputs), 13)
        self.assertEqual([data['frame'] for data in inputs], list(range(101, 114)))
        self.assertTrue(all(data['request'] == {'cmd': 'press_timeline',
            'buttons': ['b'], 'advance': True} for data in inputs))
        self.assertEqual(protocol.counts, {'get_state': 13, 'press_timeline': 13})
        self.assertEqual(rows[-1][1]['requested'], {'cmd': 'step_frames', 'count': 13})

    def test_menu_close_brakes_only_after_actual_close_and_preserves_b_edge(self):
        def close(state, button, edge):
            if button == 'b' and edge and state['field_menu'] is not None:
                state['field_menu'] = None
        protocol, native, rows = self.protocol(NativeFrames(
            road_state(field_menu={'kind': 'party'}), close))
        native.previous = 'b'  # release is necessary even after a prior B tap
        reply = protocol.drive([None, 'b', None], frames=11)
        self.assertEqual(native.buttons, [None, 'b'] + ['b'] * 9)
        self.assertIsNone(native.state['field_menu'])
        self.assertEqual(reply['data']['frame_count'], 111)
        self.assertEqual(rows[-1][1]['brake_frames'], 9)

    def test_start_opens_menu_without_added_b_cancelling_it(self):
        def open_menu(state, button, edge):
            if button == 'start' and edge:
                state['field_menu'] = {'kind': 'start'}
            if button == 'b' and edge and state['field_menu']:
                state['field_menu'] = None
        protocol, native, _ = self.protocol(NativeFrames(transition=open_menu))
        protocol.drive([None, 'start', None], frames=15)
        self.assertEqual(native.buttons, ['b', 'start'] + [None] * 13)
        self.assertEqual(native.state['field_menu'], {'kind': 'start'})

    def test_each_modal_and_unknown_phase_keeps_neutral_not_cancel(self):
        blockers = {'screen': 'battle', 'warp_fade': 'FadingOut', 'script_running': True,
            'script_awaiting_battle': True, 'fishing_active': True,
            'door_exit_pending': True, 'dialogue_state': 'WaitingForButton',
            'field_menu': {'kind': 'party'}, 'choice': {'options': ['Yes', 'No']},
            'active_script_effect': 'Delay', 'evolution_phase': 'Animating',
            'npc_trade_phase': 'Showing'}
        for key, value in blockers.items():
            for mode in ('blocked', 'missing'):
                with self.subTest(key=key, mode=mode):
                    state = road_state(**{key: value})
                    if mode == 'missing':
                        del state[key]
                    protocol, native, _ = self.protocol(NativeFrames(state))
                    protocol.step(4)
                    self.assertEqual(native.buttons, [None] * 4)

    def test_modal_acquiring_input_during_wait_stops_braking_next_frame(self):
        def enter_dialogue(state, button, edge):
            if state['frame_count'] == 101:
                state['dialogue_state'] = 'WaitingForButton'
        protocol, native, _ = self.protocol(NativeFrames(transition=enter_dialogue))
        protocol.step(6)
        self.assertEqual(native.buttons, ['b', 'b'] + [None] * 4)

    def test_modal_ending_during_wait_brakes_next_frame(self):
        def finish_script(state, button, edge):
            if state['frame_count'] == 101:
                state['script_running'] = False
                state['active_script_effect'] = None
        protocol, native, _ = self.protocol(NativeFrames(
            road_state(script_running=True, active_script_effect='Delay'), finish_script))
        protocol.step(6)
        self.assertEqual(native.buttons, [None, None] + ['b'] * 4)

    def test_requested_directions_remain_identical_and_only_tail_brakes(self):
        protocol, native, rows = self.protocol()
        reply = protocol.drive(['up'] * 8, frames=12)
        self.assertEqual(native.buttons, ['up'] * 8 + ['b'] * 4)
        self.assertEqual(reply['data']['frame_count'], 112)
        self.assertEqual(rows[-1][1]['native_requests'], 5)
        self.assertEqual(rows[-1][1]['brake_frames'], 4)

    def test_battle_ending_in_neutral_tail_and_map_change_are_reobserved(self):
        def finish(state, button, edge):
            if state['frame_count'] == 101:
                state['screen'] = 'overworld'
            if state['frame_count'] == 103:
                state['map_name'] = 'Route18'
        protocol, native, _ = self.protocol(NativeFrames(road_state(screen='battle'), finish))
        protocol.drive(['a'], frames=7)
        self.assertEqual(native.buttons, ['a', None, 'b', 'b', None, None, None])

    def test_other_maps_keep_buttons_frames_and_idle_wait_single_rpc(self):
        for name in ('Route16', 'Route18', 'PalletTown'):
            with self.subTest(name=name):
                protocol, native, rows = self.protocol(NativeFrames(road_state(map_name=name)))
                protocol.drive([None, 'b', None], frames=11)
                self.assertEqual(native.buttons, [None, 'b'] + [None] * 9)
                self.assertEqual(rows[-1][1]['brake_frames'], 0)
                self.assertEqual(len([data for kind, data in rows if kind == 'native_input']), 3)
                protocol.step(10)
                self.assertEqual(native.requests[-1], {'cmd': 'step_frames', 'count': 10})

    def test_continue_and_connection_entering_slope_recheck_trailing_neutral(self):
        for changes, button in (({'screen': 'main-menu', 'map_name': 'PalletTown'}, 'a'),
                                ({'map_name': 'Route18'}, 'up')):
            with self.subTest(changes=changes):
                def enter(state, pressed, edge):
                    if pressed == button:
                        state['screen'] = 'overworld'
                        state['map_name'] = 'Route17'
                protocol, native, _ = self.protocol(NativeFrames(road_state(**changes), enter))
                protocol.drive([None, button, None], frames=11)
                self.assertEqual(native.buttons, [None, button] + ['b'] * 9)

    def test_busy_non_slope_wait_can_land_on_slope_and_then_brakes(self):
        def finish_warp(state, button, edge):
            if state['frame_count'] == 101:
                state['map_name'] = 'Route17'
                state['warp_fade'] = 'Idle'
        protocol, native, _ = self.protocol(NativeFrames(
            road_state(map_name='Route18', warp_fade='FadingIn'), finish_warp))
        protocol.step(6)
        self.assertEqual(native.buttons, [None, None] + ['b'] * 4)

    def test_non_neutral_timeline_and_legacy_verifier_remain_single_atomic_rpc(self):
        native = NativeFrames()
        record = Mock()
        protocol = ObservedProtocol(native, record, time.monotonic() + 60)
        protocol.drive([None, 'a', None])
        self.assertEqual(len(native.requests), 1)
        protocol.brake_cycling_road = True
        protocol.drive(['left', 'left', 'b', 'b'])
        self.assertEqual(len(native.requests), 2)
        self.assertEqual(native.buttons[-4:], ['left', 'left', 'b', 'b'])

    def test_cooperative_stop_and_deadline_prevent_even_phase_reads(self):
        for stopped in (True, False):
            protocol, native, _ = self.protocol()
            protocol.stop_requested = stopped
            if not stopped:
                protocol.deadline = time.monotonic() - 1
            with self.assertRaisesRegex(StoryStopped,
                    'interrupted_at_command_boundary' if stopped else 'wall_budget'):
                protocol.step(10)
            self.assertEqual(native.requests, [])

    def test_stop_mid_wait_retains_actual_prefix_and_never_claims_complete(self):
        protocol, native, rows = self.protocol()
        def record(kind, **data):
            rows.append((kind, data))
            if kind == 'native_input':
                protocol.stop_requested = True
        protocol.record = record
        with self.assertRaisesRegex(StoryStopped, 'interrupted_at_command_boundary'):
            protocol.step(10)
        self.assertEqual(native.buttons, ['b'])
        self.assertEqual([kind for kind, _ in rows], ['native_input'])

    def test_malformed_ack_does_not_create_successful_aggregate(self):
        protocol, native, rows = self.protocol()
        send = native.cmd
        def malformed(**request):
            reply = send(**request)
            if request['cmd'] == 'press_timeline':
                reply['data']['queue_start_frame'] = 99
            return reply
        native.cmd = malformed
        with self.assertRaisesRegex(StoryStopped, 'cycling_input_chunk_not_advanced_atomically'):
            protocol.step(10)
        self.assertEqual(native.buttons, ['b'])
        self.assertEqual([kind for kind, _ in rows], ['native_input'])

    def test_bad_state_scheduled_input_and_forbidden_command_never_advance(self):
        protocol, native, _ = self.protocol()
        with self.assertRaisesRegex(StoryStopped, 'scheduled_cycling_input'):
            protocol.cmd(cmd='press_timeline', buttons=[None], start_at_frame=102)
        with self.assertRaisesRegex(StoryStopped, 'forbidden_debug_command'):
            protocol.cmd(cmd='warp', map='Route17')
        self.assertEqual(native.buttons, [])
        native.state['frame_count'] = True
        with self.assertRaisesRegex(StoryStopped, 'cycling_input_state_unavailable'):
            protocol.step(1)
        self.assertEqual(native.buttons, [])

    def test_jev_game_tap_step_and_dual_tap_share_phase_checked_protocol(self):
        protocol, native, _ = self.protocol()
        game = JevGame.__new__(JevGame)
        game.d = protocol
        game.smart_moves = True
        game.tap('a', 8)
        game.step(2)
        self.assertEqual(native.buttons, ['b', 'a'] + ['b'] * 11)
        client = AgentClient.__new__(AgentClient)
        client.d = protocol
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.client = client
        agent.tap('a')
        self.assertEqual(native.buttons[-16:], ['b', 'a'] + ['b'] * 14)
        result = client.drive(['up'] * 8, frames=12)
        self.assertEqual(result['frames'], 12)
        self.assertEqual(native.buttons[-12:], ['up'] * 8 + ['b'] * 4)

    def test_invalid_frame_count_rejected_without_inputs(self):
        protocol, native, _ = self.protocol()
        for count in (True, 1.5, -1):
            with self.assertRaises(ValueError):
                protocol.drive(['a'], frames=count)
        self.assertEqual(native.requests, [])

    def test_invalid_button_preserves_native_whole_request_validation(self):
        raw, record = Mock(), Mock()
        raw.cmd.return_value = {'ok': False, 'error': 'unknown button'}
        protocol = ObservedProtocol(raw, record, time.monotonic() + 60,
                                    brake_cycling_road=True)
        result = protocol.cmd(cmd='press_timeline', buttons=['up', None, 'invalid'])
        self.assertFalse(result['ok'])
        raw.cmd.assert_called_once_with(cmd='press_timeline',
            buttons=['up', None, 'invalid'], advance=True)

    def test_rejected_chunk_or_changed_observation_never_advances_more(self):
        for rejected in (True, False):
            protocol, native, rows = self.protocol()
            send = native.cmd
            def fail(**request):
                if rejected and request['cmd'] == 'press_timeline':
                    return {'ok': False, 'error': 'synchronous timeline requires an empty input queue'}
                result = send(**request)
                if not rejected and request['cmd'] == 'get_state' and native.buttons:
                    result['data']['frame_count'] += 1
                return result
            native.cmd = fail
            if rejected:
                self.assertFalse(protocol.step(3)['ok'])
                self.assertEqual(native.buttons, [])
            else:
                with self.assertRaisesRegex(StoryStopped, 'cycling_input_state_frame_changed'):
                    protocol.step(3)
                self.assertEqual(native.buttons, ['b'])
            self.assertTrue(all(kind == 'native_input' for kind, _ in rows))


if __name__ == '__main__':
    unittest.main()
