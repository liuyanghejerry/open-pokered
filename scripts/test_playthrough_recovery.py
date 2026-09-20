"""Regression scenarios for forest, Silph rival and league blackout recovery."""
import unittest
from unittest.mock import Mock, patch
import playthrough_late as late
import playthrough as pt
import sys


class ForestRecoveryTests(unittest.TestCase):
    @patch.object(pt,'walk_pallet_to_pewter')
    def test_real_blackout_advances_existing_training_before_retry(self,walk):
        walk.side_effect=[pt.NavError('warped out'),None]
        game=Mock()
        game.st.return_value={'screen':'overworld','map_name':'ViridianCity',
            'battle_phase':'TrainerVictory { player_won: false }',
            'battle_live':{'player_party':[{'hp':0}]}}
        pt.m09_to_pewter(game)
        game.train_until.assert_called_once_with(13,'Route1',(12,7),
            ((23,25),'ViridianCity','ViridianPokecenter'))
        self.assertEqual(walk.call_count,2)

    @patch.object(pt,'walk_pallet_to_pewter',side_effect=pt.NavError('unexpected warp'))
    def test_city_location_without_blackout_does_not_trigger_training(self,_):
        game=Mock()
        game.st.return_value={'screen':'overworld','map_name':'ViridianCity',
            'battle_phase':'PlayerMenu','battle_live':{'player_party':[{'hp':20}]}}
        with self.assertRaises(pt.NavError):pt.m09_to_pewter(game)
        game.train_until.assert_not_called()


class SilphRecoveryTests(unittest.TestCase):
    def game(self, losses):
        game=Mock()
        game.losses=losses;game.attempt=0
        def enter(*args):game.attempt+=1
        game.nav_to.side_effect=enter
        def state():
            lost=game.attempt<=losses
            return {'screen':'overworld','map_name':'SaffronCity' if lost else 'SilphCo7F',
                    'battle_live':{'player':{'hp':0 if lost else 20}},
                    'battle_phase':f'TrainerVictory {{ player_won: {str(not lost).lower()} }}'}
        game.st.side_effect=state
        def flags(**request):
            return {'data':{'EVENT_SILPH_CO_3_UNLOCKED_DOOR2':True,
                            'EVENT_BEAT_SILPH_CO_RIVAL':game.attempt>losses}}
        game.d.cmd.side_effect=flags
        return game

    @patch.object(late,'talk_object')
    def test_blackout_reenters_earned_route_then_accepts_actual_win(self,_):
        game=self.game(1)
        late.challenge_silph_rival(game)
        self.assertEqual(game.attempt,2)
        game.heal_pokecenter.assert_called_once_with((9,29),'SaffronCity','SaffronPokecenter')
        self.assertEqual(game.nav_warp.call_count,8)

    @patch.object(late,'talk_object')
    def test_repeated_losses_are_bounded(self,_):
        game=self.game(99)
        with self.assertRaisesRegex(AssertionError,'EVENT_BEAT_SILPH_CO_RIVAL'):
            late.challenge_silph_rival(game,max_attempts=3)
        self.assertEqual(game.attempt,3)
        self.assertEqual(game.heal_pokecenter.call_count,2)

    @patch.object(late,'talk_object')
    def test_driver_error_without_blackout_is_not_retried(self,_):
        game=self.game(0);game.nav_to.side_effect=RuntimeError('door path disconnected')
        game.st.return_value={};game.st.side_effect=lambda:{'screen':'overworld','map_name':'SilphCo3F','battle_live':{'player':{'hp':151}},'battle_phase':''}
        with self.assertRaisesRegex(RuntimeError,'door path disconnected'):
            late.challenge_silph_rival(game)
        game.heal_pokecenter.assert_not_called()
        self.assertEqual(game.nav_to.call_count,1)

    @patch.object(late,'talk_object')
    def test_stale_zero_hp_without_loss_phase_is_not_a_retry(self,_):
        game=self.game(1)
        game.st.side_effect=lambda:{'screen':'overworld','map_name':'SaffronCity','battle_live':{'player':{'hp':0}},'battle_phase':'PlayerMenu'}
        with self.assertRaises(AssertionError):late.challenge_silph_rival(game)
        self.assertEqual(game.attempt,1)
        game.heal_pokecenter.assert_not_called()

class LeagueRecoveryTests(unittest.TestCase):
    def state(self, hp=99, status='None', place='LoreleisRoom'):
        player={'species':'Zapdos','hp':hp,'max_hp':157,'status':status,'level':51}
        return {'map_name':place,'screen':'battle','battle_live':{
            'player':player,'player_party':[player],
            'enemy':{'species':'Dewgong','hp':6,'max_hp':169}},
            'battle_inventory':[{'item':'FullRestore','qty':16}]}

    def test_does_not_heal_lock_at_recorded_dewgong_state(self):
        self.assertIsNone(late.battle_recovery_plan(self.state()))
        self.assertEqual(late.battle_recovery_plan(self.state(place='SilphCo7F')),('FullRestore',0))

    def test_still_treats_critical_hp_and_status(self):
        self.assertEqual(late.battle_recovery_plan(self.state(hp=40)),('FullRestore',0))
        self.assertEqual(late.battle_recovery_plan(self.state(hp=157,status='Freeze')),('FullRestore',0))

    @patch.object(late,'lead_with')
    def test_confirmed_blackout_walks_back_without_injected_supplies(self,lead):
        game=Mock();state=self.state(hp=0,place='IndigoPlateau')
        state.update(screen='overworld',battle_phase='TrainerVictory { player_won: false }',money=1000)
        game.st.return_value=state;game.d.cmd.return_value={'data':[]}
        self.assertTrue(late.retry_elite_four(game,'m47',0))
        game.nav_warp.assert_called_once_with(9,5,'IndigoPlateau','IndigoPlateauLobby')
        lead.assert_called_once_with(game,'Zapdos')
        self.assertFalse(late.retry_elite_four(game,'m47',5))
        state['battle_phase']='PlayerMenu'
        self.assertFalse(late.retry_elite_four(game,'m47',0))

    @patch.object(late,'use_item')
    def test_exhausted_inventory_does_not_invent_medicine(self,use):
        game=Mock();game.st.return_value={'party':[{'hp':0,'max_hp':10,'status':'None'}]}
        game.d.cmd.return_value={'data':[]}
        late.recover_party(game);use.assert_not_called()

    def test_dispatcher_replays_lorelei_after_later_league_loss(self):
        events=[];failed=[False]
        def stage(name):
            def run(game):
                events.append(name)
                if name=='m46' and not failed[0]:failed[0]=True;raise AssertionError('blackout')
            return run
        stages=[(name,name,stage(name)) for name in ('m44','m45','m46','m47')]
        game=Mock();game.persistent=False
        with patch.object(pt,'Game',return_value=game) as cls,patch.object(pt,'MILESTONES',stages),\
             patch.object(late,'retry_elite_four',return_value=True) as retry,\
             patch.object(sys,'argv',['playthrough.py']):
            cls.milestone_index.side_effect=lambda name:int(name[1:])
            pt.main()
        self.assertEqual(events,['m44','m45','m46','m45','m46','m47'])
        retry.assert_called_once_with(game,'m46',0)
        game.close.assert_called_once()

    @patch.object(late,'recover_party')
    def test_champion_blackout_detected_after_delayed_warp(self,_):
        game=Mock()
        entering={'hall_of_fame_count':0}
        settling={'screen':'overworld','map_name':'ChampionsRoom'}
        lost=self.state(hp=0,place='IndigoPlateau')
        lost.update(screen='overworld',battle_phase='TrainerVictory { player_won: false }')
        game.st.side_effect=[entering,settling,lost]
        with self.assertRaisesRegex(RuntimeError,'confirmed blackout'):
            late.m49_first_clear(game)
        game.step.assert_not_called()


if __name__=='__main__':unittest.main()
