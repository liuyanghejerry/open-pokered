"""Scoring and deadline contracts, independent of either model service."""
import io
import json
import struct
import unittest
from types import SimpleNamespace

from openpokered.evaluation import EvaluationBudget, EvaluationMetrics, EvaluationStopped, controller_response
from openpokered.evaluation_models import MeasuredModel, tcp_rtt_seconds
from openpokered.typesafe import Choice, SystemOneResult


class BudgetTests(unittest.TestCase):
    def setUp(self):
        self.now=0.
        self.budget=EvaluationBudget(20,3,lambda:self.now)

    def test_warmup_is_outside_clock_and_cannot_earn_credit(self):
        self.now=100
        self.assertEqual(self.budget.raw(),0)
        self.assertEqual(self.budget.add_rtt(.2,1),0)
        self.budget.start();self.now+=5
        self.assertEqual(self.budget.raw(),5)

    def test_credit_uses_rtt_not_inference_duration_and_is_bounded(self):
        self.budget.start();self.now=20.1
        self.assertEqual(self.budget.add_rtt(.2,5),.2)
        self.budget.check()
        self.assertAlmostEqual(self.budget.effective(),19.9)
        self.assertEqual(self.budget.add_rtt(10,1),1)
        self.assertEqual(self.budget.add_rtt(10,20),1.8)
        self.assertEqual(self.budget.add_rtt(1,1),0)
        self.now=23
        with self.assertRaises(EvaluationStopped):self.budget.check()

    def test_unsupported_and_invalid_samples_receive_no_credit(self):
        self.budget.start()
        for sample in (None,-1,float('nan'),float('inf')):
            self.assertEqual(self.budget.add_rtt(sample,2),0)

    def test_finish_at_zero_freezes_the_clock(self):
        self.budget.start();self.budget.finish();self.now=8
        self.assertEqual(self.budget.raw(),0)

    def test_darwin_socket_value_is_milliseconds_at_sdk_offset(self):
        info=bytearray(104);struct.pack_into('=I',info,40,123)
        sock=SimpleNamespace(getsockopt=lambda *args:bytes(info))
        response=SimpleNamespace(fp=SimpleNamespace(raw=SimpleNamespace(_sock=sock)))
        self.assertEqual(tcp_rtt_seconds(response,'darwin'),.123)
        self.assertIsNone(tcp_rtt_seconds(response,'linux'))
        self.assertIsNone(tcp_rtt_seconds(object(),'darwin'))

    def test_late_judgment_is_logged_but_cannot_drive_an_action(self):
        self.budget.start()
        def infer(*args,**kwargs):
            self.now=21
            return SystemOneResult('test',{},123,0)
        model=MeasuredModel.__new__(MeasuredModel)
        model.backend='jev';model.budget=self.budget;model.journal=io.StringIO()
        model.status_callback=lambda:None;model.records=[];model.opener=None
        model.inner=SimpleNamespace(system_one=infer);model.model='test'
        with self.assertRaises(EvaluationStopped):
            model.system_one({'hp':1},{'action':Choice('Choose',{'heal':'Heal'})})
        record=json.loads(model.journal.getvalue())
        self.assertEqual(record['input_tokens'],123)
        self.assertEqual(record['state'],{'hp':1})
        self.assertTrue(record['success'])


class ScoringTests(unittest.TestCase):
    def setUp(self):
        self.metrics=EvaluationMetrics([{'id':'starter','satisfied_when':{'flag':'STARTER'}}])
        self.state={'map_name':'PalletTown','player_x':1,'player_y':2,'screen':'overworld',
                    'frame_count':1,'money':3000,'badges':0,'party':[
                        {'species':'Bulbasaur','level':5,'hp':20,'max_hp':20,'total_exp':135}],
                    'pokedex':{'seen':1,'owned':1,'total':151,'owned_numbers':[1]}}

    def observe(self,elapsed,flags=None,bag=None):
        return self.metrics.observe(self.state,flags or {},bag or [],{
            'effective_s':elapsed,'raw_s':elapsed+1,'rtt_credit_s':1,'limit_s':1200})

    def test_progress_and_optional_completion_are_earned_not_visits(self):
        self.observe(0);self.state['map_name']='PowerPlant';self.observe(5,{'STARTER':True})
        self.assertEqual(self.metrics.milestones['starter']['effective_s'],5)
        self.assertNotIn('zapdos',self.metrics.side_completed)
        self.state['pokedex']['owned_numbers'].append(145)
        self.observe(10,bag=[{'item':'Hm02','qty':1}])
        self.assertEqual(self.metrics.side_completed,{'fly':10,'zapdos':10})

    def test_deadline_excludes_late_progress(self):
        self.observe(1199)
        self.assertIsNone(self.observe(1201,{'STARTER':True}))
        self.assertFalse(self.metrics.milestones)
        self.assertEqual(len(self.metrics.timeline),1)

    def test_title_screen_default_map_does_not_count_as_a_visit(self):
        self.state['screen']='title';self.observe(0)
        self.assertEqual(self.metrics.maps,set())

    def test_live_battle_telemetry_wins_over_stale_save_party(self):
        self.state['screen']='battle'
        mon={**self.state['party'][0],'hp':0,'total_exp':200}
        self.state['evaluation']={'party':[mon],'pokedex':self.state['pokedex'],'party_source':'battle_live'}
        row=self.observe(3)
        self.assertEqual(row['party'][0]['hp'],0)
        self.assertEqual(row['party'][0]['total_exp'],200)
        self.assertEqual(row['failures']['observed_party_wipes'],1)

    def test_measurements_do_not_leak_into_controller_inputs(self):
        for data in ({'screen':'battle','evaluation':{'party':['private']}},
                     {'state':{'screen':'battle','evaluation':{'party':['private']}},'other':42}):
            response={'ok':True,'data':data};before=json.dumps(response)
            filtered=controller_response(response)
            state=filtered['data'].get('state',filtered['data'])
            self.assertNotIn('evaluation',state)
            self.assertEqual(json.dumps(response),before)
            self.assertEqual(state['screen'],'battle')

    def test_money_separates_initial_balance_income_and_spending(self):
        self.observe(0);self.state['money']=3300;self.observe(1)
        self.state['money']=2900;self.observe(2)
        money=self.metrics.summary()['money']
        self.assertEqual((money['initial'],money['final'],money['peak']),(3000,2900,3300))
        self.assertEqual((money['observed_positive_deltas'],money['observed_negative_deltas']),(300,400))

    def test_flag_toggles_healing_and_backtracking_do_not_hide_a_stall(self):
        self.observe(0,{'TRANSIENT':True})
        self.state['party'][0]['hp']=1;self.observe(100)
        self.state['party'][0]['hp']=20;self.observe(181,{'TRANSIENT':True})
        self.assertTrue(self.metrics.stall_active)
        self.state['party'][0]['total_exp']+=10;self.observe(190)
        self.assertFalse(self.metrics.stall_active)
        self.assertEqual(self.metrics.stalls[0]['ended_s'],190)

    def test_failure_categories_are_separate(self):
        self.metrics.event('battle_defeat',{})
        self.metrics.event('outcome',{'result':'blocked','operation':'travel_to:MtMoon1F'})
        self.metrics.event('outcome',{'result':'reached','operation':'travel_to:MtMoon1F'})
        self.assertEqual(dict(self.metrics.failure_counts),{
            'reported_battle_defeats':1,'unsuccessful_operations':1,'blocked_travel_operations':1})


if __name__=='__main__':unittest.main()
