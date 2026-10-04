import copy
import unittest

from scripts.test_openpokered_collection_verification import observations
from scripts.openpokered.boxed_stat_handover import validate_boxed_stat_handover


def checksummed(saved):
    saved = bytearray(saved)
    for bank in (2, 3):
        start, total = bank * 8192, 6 * 1122
        saved[start + total] = (~sum(saved[start:start + total])) & 255
        for box in range(6):
            offset = start + box * 1122
            saved[start + total + 1 + box] = (~sum(saved[offset:offset + 1122])) & 255
    return bytes(saved)


def fixture():
    before = observations(1)
    state = before['get_state']['data']
    state['box_counts'] = [1] + [0] * 11
    state['current_box_index'] = 0
    state['stored_pokemon'] = [{'box': 0, 'index': 0, 'species': 'Hypno',
        'level': 38, 'hp': 129, 'max_hp': 129, 'status': 'None',
        'moves': ['None'] * 4, 'pp': [0] * 4}]
    after = copy.deepcopy(before)
    after['get_state']['data']['stored_pokemon'][0]['max_hp'] = 130
    saved = bytearray(32768)
    for box in range(12):
        start = (2 + box // 6) * 8192 + (box % 6) * 1122
        saved[start + 1] = 255
    saved[16384:16387] = bytes([1, 97, 255])
    record = bytearray(33)
    record[0:5] = bytes([97, 0, 129, 38, 0])
    record[14:17] = (59108).to_bytes(3, 'big')
    for index, ev in enumerate((7864, 12837, 7647, 13972, 6908)):
        record[17 + 2 * index:19 + 2 * index] = ev.to_bytes(2, 'big')
    record[27:29] = bytes([147, 142])
    saved[16406:16439] = record
    return checksummed(saved), before, after


class BoxedStatHandoverTests(unittest.TestCase):
    def test_read_only_raw_formula_witness_is_not_historical_strict_pass(self):
        saved, before, after = fixture()
        original = copy.deepcopy((before, after))
        proof = validate_boxed_stat_handover(saved, before, after)
        self.assertEqual((before, after), original)
        self.assertTrue(proof['cache_handover_verified'])
        self.assertFalse(proof['historical_strict_continue_verified'])
        self.assertEqual(len(proof['saved_box_records']), 1)
        change = proof['changes'][0]
        self.assertEqual((change['legacy'], change['restored'], change['current_hp']), (129, 130, 129))
        self.assertEqual(change['derived_stats'], [130, 77, 68, 73, 111])

    def test_every_other_exposed_field_is_strict(self):
        mutations = [
            lambda o: o['get_state']['data'].update(money=1),
            lambda o: o['get_state']['data'].update(player_x=6),
            lambda o: o['get_state']['data']['safari_game'].update(active=True),
            lambda o: o['get_bag']['data'].clear(),
            lambda o: o['get_flags']['data'].clear(),
            lambda o: o['get_party']['data'][0].update(experience=216001),
            lambda o: o['get_state']['data']['party'][0]['pp'].__setitem__(0, 17),
        ]
        for mutate in mutations:
            saved, before, after = fixture()
            mutate(after)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                validate_boxed_stat_handover(saved, before, after)

    def test_no_healing_or_stored_identity_move_pp_level_changes(self):
        for field, value in [('hp', 130), ('species', 'Drowzee'), ('level', 39),
                             ('status', 'Poison'), ('pp', [1, 0, 0, 0]),
                             ('moves', ['Tackle', 'None', 'None', 'None']), ('index', 1)]:
            saved, before, after = fixture()
            after['get_state']['data']['stored_pokemon'][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_boxed_stat_handover(saved, before, after)

    def test_agreed_visible_values_must_still_match_saved_record(self):
        for field, value in [('hp', 128), ('species', 'Drowzee'), ('level', 39),
                             ('status', 'Poison'), ('pp', [1, 0, 0, 0]),
                             ('pp', [False, 0, 0, 0])]:
            saved, before, after = fixture()
            for observation in (before, after):
                observation['get_state']['data']['stored_pokemon'][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_boxed_stat_handover(saved, before, after)

    def test_target_max_must_follow_dv_ev_formula_not_an_allowlisted_number(self):
        for offset, value in [(16406 + 27, 0), (16406 + 17, 255)]:
            saved, before, after = fixture()
            damaged = bytearray(saved)
            damaged[offset] = value
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                validate_boxed_stat_handover(checksummed(damaged), before, after)
        saved, before, after = fixture()
        after['get_state']['data']['stored_pokemon'][0]['max_hp'] = 131
        with self.assertRaisesRegex(ValueError, 'original formula'):
            validate_boxed_stat_handover(saved, before, after)

    def test_invalid_source_counts_sentinel_species_status_move_and_level(self):
        for offset, value in [(16384, 2), (16386, 0), (16385, 96),
                              (16406, 255), (16410, 128), (16414, 255), (16409, 0)]:
            saved, before, after = fixture()
            damaged = bytearray(saved)
            damaged[offset] = value
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                validate_boxed_stat_handover(checksummed(damaged), before, after)
        with self.assertRaises(ValueError):
            validate_boxed_stat_handover(saved[:-1], before, after)

    def test_unchanged_or_invalid_max_cannot_be_used_as_a_migration(self):
        saved, before, after = fixture()
        with self.assertRaisesRegex(ValueError, 'strict CONTINUE'):
            validate_boxed_stat_handover(saved, after, after)
        for value in (True, 0, 1000, '129'):
            before['get_state']['data']['stored_pokemon'][0]['max_hp'] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_boxed_stat_handover(saved, before, after)

    def test_original_eight_bit_ev_cap(self):
        saved, before, after = fixture()
        damaged = bytearray(saved)
        damaged[16423:16425] = bytes([255, 255])
        damaged[16409] = 100
        # At lv100, the 8-bit cap is distinguishable from a 256-root bonus.
        for observation in (before, after):
            observation['get_state']['data']['stored_pokemon'][0]['level'] = 100
        before['get_state']['data']['stored_pokemon'][0]['max_hp'] = 366
        after['get_state']['data']['stored_pokemon'][0]['max_hp'] = 367
        proof = validate_boxed_stat_handover(checksummed(damaged), before, after)
        self.assertEqual(proof['changes'][0]['derived_stats'][0], 367)

    def test_corrupted_box_bank_checksum_is_rejected(self):
        saved, before, after = fixture()
        for offset in (16384 + 6 * 1122, 16384 + 6 * 1122 + 1, 24576 + 6 * 1122):
            damaged = bytearray(saved)
            damaged[offset] ^= 1
            with self.subTest(offset=offset), self.assertRaisesRegex(ValueError, 'checksum'):
                validate_boxed_stat_handover(bytes(damaged), before, after)


if __name__ == '__main__':
    unittest.main()
