"""Item-name regressions: generated fixtures must use serde's exact variants."""
import unittest
from save_builder import SaveBuilder


def builder():
    result = SaveBuilder.__new__(SaveBuilder)
    result.data = {'game_data': {'bag': {'items': []}}}
    return result


class ItemNameRegression(unittest.TestCase):
    def test_debug_names_and_constants_merge_into_the_same_stack(self):
        bag = builder()
        for debug, constant in [('HelixFossil','HELIX_FOSSIL'),('SsTicket','S_S_TICKET'),
                                ('LiftKey','LIFT_KEY'),('SilphScope','SILPH_SCOPE'),
                                ('PpUp','PP_UP'),('Hm01','HM_01'),('Tm24','TM_24')]:
            bag.give_item(debug,2).give_item(constant,3)
        self.assertEqual(bag.data['game_data']['bag']['items'],[
            ['HelixFossil',5],['SsTicket',5],['LiftKey',5],['SilphScope',5],
            ['PpUp',5],['Hm01',5],['Tm24',5]])

    def test_unknown_item_is_rejected_before_mutating_the_fixture(self):
        bag = builder().give_item('POTION',1)
        for name in ['DefinitelyNotAnItem','TM_51','HM_06']:
            with self.assertRaises(KeyError):bag.give_item(name,1)
        self.assertEqual(bag.data['game_data']['bag']['items'],[['Potion',1]])


if __name__=='__main__':unittest.main()
