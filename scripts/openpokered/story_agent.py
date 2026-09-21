"""Two Jev decision layers over condition-preserving scene rules.

Strategy selects an attainable story subgoal. Action selects its live trigger or
menu option. Real input executes it; only observed game facts establish success.
"""
import json
import time
from collections import Counter

from . import skills
from .story_rules import StoryIndex, normalize_bag
from .typesafe import Choice, TypeSafeError


class StoryStopped(RuntimeError):
    pass


def progress_key(facts):
    # Location and open dialogue alone are not story progress. Failed actions
    # become eligible again when their relevant world/party state has changed.
    return json.dumps({**{k: facts[k] for k in ('flags', 'bag', 'party', 'badges')},
                       'cleared_terrain': facts.get('cleared_terrain', []),
                       'navigation_revision': facts.get('navigation_revision', 0)}, sort_keys=True)


def attempt_key(rule, facts):
    # Different conditional paths can describe the same physical trigger.
    # A blocked interaction should not receive a fresh retry allowance for
    # every predicted effect of that identical interaction.
    return (json.dumps([rule.map, rule.storyline, rule.triggers, rule.choices, facts.get('map')], sort_keys=True),
            progress_key(facts))


class DualStoryAgent:
    def __init__(self, client, model_client, objectives, *, model='jev-1.13.0',
                 strategy_jev=True, action_jev=True, max_calls=160,
                 max_actions=180, frame_budget=80000, wall_budget=600,
                 maps_dir=None, trace=None):
        if not objectives or any(not o.get('satisfied_when', {}).get('flag') for o in objectives):
            raise ValueError('nonempty objectives with explicit completion flags required')
        self.client = client
        self.model_client = model_client
        self.objectives = objectives
        self.model = model
        self.layer_jev = {'strategy': strategy_jev, 'action': action_jev}
        self.max_calls, self.max_actions = max_calls, max_actions
        self.frame_budget, self.wall_budget = frame_budget, wall_budget
        self.maps_dir, self.trace = maps_dir, trace
        self.calls = Counter()
        self.tokens = Counter()
        self.models = set()
        self.actions = 0
        self.resolved_battles = 0
        self.travel_battles = 0
        self.failures = Counter()
        self.recent = []
        self.active = None
        self.completed = []
        self.start_frame = 0
        self.start_time = time.monotonic()
        self.index = None

    def record(self, kind, **payload):
        event = {'kind': kind, 'elapsed_s': round(time.monotonic() - self.start_time, 3), **payload}
        if self.trace:
            self.trace.write(json.dumps(event, ensure_ascii=False) + '\n')
            self.trace.flush()
        if kind in ('strategy', 'milestone', 'stopped'):
            print(json.dumps(event, ensure_ascii=False), flush=True)

    def check_budget(self):
        if time.monotonic() - self.start_time >= self.wall_budget:
            raise StoryStopped('wall_budget')
        if self.client.state()['frame_count'] - self.start_frame >= self.frame_budget:
            raise StoryStopped('frame_budget')

    def facts(self):
        state = self.client.state()
        return {'flags': self.client.flags(), 'bag': normalize_bag(self.client.bag()),
                'party': [{k: m.get(k) for k in ('species', 'level', 'hp', 'max_hp', 'status')}
                          for m in state.get('party', [])],
                'badges': self.client.observe().get('badges', {}).get('count', 0),
                'map': state['map_name'], 'x': state['player_x'], 'y': state['player_y'],
                'money': state.get('money'), 'coins': state.get('coins')}

    def choose(self, layer, state, candidates, instruction, *, allow_abstain=True):
        if not candidates:
            raise StoryStopped(f'{layer}:no_candidates')
        if not self.layer_jev[layer]:
            return next(iter(candidates))
        if sum(self.calls.values()) >= self.max_calls:
            raise StoryStopped('judgment_cap')
        self.check_budget()
        criteria = dict(candidates)
        if allow_abstain:
            criteria['none'] = 'None of these candidates can advance the current goal.'
        question = Choice(instruction, criteria)
        self.calls[layer] += 1
        started = time.monotonic()
        try:
            result = self.model_client.system_one(state, {layer: question}, model=self.model)
        except TypeSafeError as e:
            self.record('judgment_error', layer=layer, error=str(e))
            raise StoryStopped(f'{layer}:service_unavailable') from e
        answer = result.answers.get(layer)
        self.tokens[layer] += result.input_tokens
        self.tokens['output'] += result.output_tokens
        self.models.add(result.model)
        self.record('judgment', layer=layer, state=state, question=question.to_json(),
                    answer=vars(answer) if answer else None, model=result.model,
                    input_tokens=result.input_tokens, output_tokens=result.output_tokens,
                    latency_s=round(time.monotonic() - started, 3))
        if allow_abstain and answer and answer.choice == 'none':
            supported = {key: answer.probabilities.get(key, 0) for key in candidates}
            # Similar valid choices split their probability mass. Abstain
            # only when "none" outweighs the alternatives collectively.
            if sum(supported.values()) > answer.probabilities.get('none', 1):
                selected = max(supported, key=supported.get)
                self.record('conditional_choice', layer=layer, selected=selected,
                            candidate_mass=sum(supported.values()),
                            abstention_mass=answer.probabilities.get('none', 1))
                return selected
        if not answer or answer.choice not in candidates:
            raise StoryStopped(f'{layer}:no_selection')
        return answer.choice

    def mark_milestones(self, facts):
        for objective in self.objectives:
            if self.objective_satisfied(objective, facts) and objective['id'] not in self.completed:
                self.completed.append(objective['id'])
                self.record('milestone', objective=objective['id'], flag=objective['satisfied_when']['flag'],
                            frame=self.client.state()['frame_count'])

    def objective_satisfied(self, objective, facts):
        return bool(facts['flags'].get(objective['satisfied_when']['flag']))

    def strategy_groups(self, facts):
        groups = {}
        for objective in self.objectives:
            target = ('flag', objective['satisfied_when']['flag'], True)
            if self.objective_satisfied(objective, facts):
                continue
            for rule in self.index.frontier(target, facts):
                if rule.effect[0] not in ('flag', 'item', 'visibility'):
                    continue
                # An empty party cannot perform a battle-producing script.
                if not facts['party'] and any(e[0] == 'battle' for e in rule.preceding):
                    continue
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'objectives': [], 'rules': []})
                if objective['name'] not in group['objectives']:
                    group['objectives'].append(objective['name'])
                if rule not in group['rules']:
                    group['rules'].append(rule)
        return groups

    def select_strategy(self, facts):
        groups = self.strategy_groups(facts)
        candidates = {}
        offered = {}
        for n, group in enumerate(groups.values()):
            group['rules'] = [r for r in group['rules'] if not self.failures[attempt_key(r, facts)] >= 2]
            if not group['rules']:
                continue
            key = f'subgoal:{n}'
            offered[key] = group
            candidates[key] = json.dumps({'establish': group['target'], 'advances': group['objectives'],
                                          'ways': [r.description() for r in group['rules'][:6]],
                                          'context': group.get('context')})
        navigation = {}
        for group in offered.values():
            for rule in group['rules']:
                if rule.map not in navigation:
                    route = self.client.route(facts['map'], rule.map)
                    navigation[rule.map] = {
                        'topological_route_found': route.get('found', False),
                        'via': [leg['to_map'] for leg in route.get('legs', [])],
                    }
        # Rule expansion already evaluated the complete world. The judgment
        # needs the facts distinguishing the offered options, not hundreds of
        # unrelated historical flags and failures from completed objectives.
        relevant = json.dumps(candidates)
        world = {**facts, 'flags': {k: v for k, v in facts['flags'].items() if k in relevant},
                 'total_completed_event_flags': sum(bool(v) for v in facts['flags'].values())}
        if 'recent_battle_defeats' in world:
            world['recent_battle_defeats'] = [d for d in world['recent_battle_defeats'] if not d.get('resolved_by_victory')]
        failures = {name: failure for name, failure in getattr(self, 'navigation_memory', {}).items()
                    if name in navigation and not self.index.satisfied(failure['goal'], facts)}
        state = {'world': world, 'completed_objectives': self.completed,
                 'remaining_objectives': [o['name'] for o in self.objectives
                                          if not self.objective_satisfied(o, facts)],
                 'navigation': navigation,
                 'known_navigation_failures': failures,
                 'recent_outcomes': self.recent[-4:]}
        selected = self.choose('strategy', state, candidates,
                               'Which attainable story or preparation subgoal should the player pursue next? '
                               'Healing, training and improving weak attacks are valid indirect progress toward later battles. '
                               'The action layer can travel between maps before triggering a script; '
                               'a subgoal need not be on the current map. Candidates already have '
                               'satisfied script preconditions; winning battles is still an execution outcome. '
                               'Topological routes may have local obstacles. Use the supplied script facts '
                               'and known navigation failures; prefer resolving an observed blocker before '
                               'retrying its destination unless prerequisites have changed. A known tile route '
                               'to the specific trigger is stronger evidence than a map-only failure: another '
                               'region on that same map can still be reachable. Prefer reachable prerequisites '
                               'when the requested trigger has no tile route. Prefer necessary early '
                               'story prerequisites before optional detours or difficult battles.')
        self.active = offered[selected]
        self.record('strategy', target=self.active['target'], advances=self.active['objectives'])

    def action_candidates(self, facts):
        candidates, bindings = {}, {}
        npcs = self.client.cmd(cmd='get_npcs')
        for rule in self.active['rules']:
            if self.failures[attempt_key(rule, facts)] >= 2:
                continue
            if rule.missing(facts):
                continue
            actions = []
            if facts['map'] != rule.map:
                actions.append(f'travel_to:{rule.map}')
            else:
                for trigger in rule.triggers:
                    if trigger.startswith('npc:'):
                        text_id = int(trigger.split(':')[1])
                        for npc in npcs:
                            if npc.get('text_id') == text_id and npc.get('visible', True):
                                actions.append(f"interact_with:npc:{npc['npc_index']}")
                    elif trigger.startswith('sign:'):
                        text_id = int(trigger.split(':')[1])
                        path = self.index.maps_dir / rule.map / 'map.json'
                        if path.exists():
                            for index, sign in enumerate(json.loads(path.read_text()).get('signs', [])):
                                if sign.get('textId') == text_id:
                                    actions.append(f'interact_with:sign:{index}')
                for x, y in self.index.coordinates(rule):
                    actions.append(f'move_to:{x},{y}')
                if 'load' in rule.triggers:
                    # The entry script may already be queued; settle first.
                    actions.append('wait_for_control')
            for action in actions:
                key = f'action:{len(candidates)}'
                candidates[key] = json.dumps({'operation': action, 'script_effects': rule.description()})
                bindings[key] = (action, rule)
        return candidates, bindings

    def settle(self, goal, rule=None):
        """Drive busy screens to control, retaining dialogue and choices."""
        menu_pick = None
        last_dialogue = None
        self.client.step(2)
        for _ in range(400):
            self.check_budget()
            state = self.client.state()
            effect = state.get('active_script_effect')
            if self.settle_special(state):
                continue
            field_menu = state.get('field_menu')
            if field_menu and field_menu.get('kind') == 'elevator':
                options = field_menu['items']
                signature = ('elevator', tuple(options))
                if menu_pick is None or menu_pick[0] != signature:
                    selected = self.choose('action', {
                        'subgoal': goal, 'script': rule.description() if rule else None,
                        'menu': options}, {str(i): label for i, label in enumerate(options)},
                        'Which elevator floor advances the selected subgoal? Use the script confirmation option and destination.')
                    menu_pick = (signature, int(selected))
                self.tap('a' if field_menu['cursor'] == menu_pick[1] else 'down')
                continue
            if state['screen'] == 'battle':
                self.record('battle_started', state=state)
                self.fight_battle()
                if self.client.state()['screen'] == 'battle':
                    raise StoryStopped('battle_did_not_finish')
                self.resolved_battles += 1
                self.record('battle_resolved', state=self.client.state())
                continue
            if state.get('choice'):
                menu = state['choice']
                options = menu['options']
                signature = (tuple(options), last_dialogue)
                if menu_pick is None or menu_pick[0] != signature:
                    candidates = {str(i): label for i, label in enumerate(options)}
                    navigation = (getattr(self, 'navigation_intent', None)
                                  if getattr(getattr(self, 'game', None), 'navigation_active', False) else None)
                    selected = self.choose('action', {
                        'subgoal': goal, 'dialogue': last_dialogue,
                        'current_map': state.get('map_name'), 'travel_in_progress': navigation,
                        'script': rule.description() if rule else None,
                        'menu': options}, candidates,
                        'Which menu option advances the current subgoal? Use the dialogue and '
                        'the script confirmation options to interpret the choice. When travelling, '
                        'choose the option that permits reaching the intended destination; a gate '
                        'or exit choice can be necessary before the destination provides the final effect.')
                    menu_pick = (signature, int(selected))
                desired = menu_pick[1]
                if menu['selected'] != desired:
                    self.tap('down')
                else:
                    self.tap('a')
                continue
            menu_pick = None
            if state.get('dialogue_state'):
                text = (state.get('script_effect') or {}).get('text') or state.get('dialogue')
                if text:
                    last_dialogue = text
                    self.record('dialogue', text=text, map=state['map_name'])
                self.client.skip_dialogue()
                continue
            if effect == 'ShowPokedexEntry':
                self.tap('a')
                continue
            obs = self.client.observe()
            if obs['mode'] == 'overworld' and not state.get('script_running') and not effect:
                return
            self.client.step(10)
        self.record('unsettled', state=self.client.state())
        raise StoryStopped('interaction_did_not_settle')

    def tap(self, button):
        # Explicit release before the press also handles the first input
        # after a preview/menu handoff re-baselines its edge detector.
        self.client.cmd(cmd='press_timeline', buttons=[None, button, None])
        self.client.step(13)

    def fight_battle(self):
        skills.battle_loop(self.client, on_round=lambda *_: self.check_budget())

    def settle_special(self, state):
        return False

    def should_replan(self, facts):
        return False

    def action_rejected(self, facts, reason):
        return False

    def execute(self, operation, rule):
        if self.actions >= self.max_actions:
            raise StoryStopped('action_budget')
        self.actions += 1
        verb, _, arg = operation.partition(':')
        if verb == 'travel_to':
            result = self.client.travel_to(arg)
        elif verb == 'interact_with':
            result = self.client.interact_with(arg)
        elif verb == 'move_to':
            result = self.client.move_to(*(int(v) for v in arg.split(',')))
        elif verb == 'wait_for_control':
            result = {'result': 'waited'}
        else:
            raise StoryStopped(f'unknown_operation:{operation}')
        # travel_to handles some encounters internally. Keep these separate
        # from battles completed explicitly by settle(), and from victories.
        self.travel_battles += result.get('battles', 0)
        self.record('operation', operation=operation, result=result,
                    subgoal=self.active['target'], script=rule.storyline)
        self.settle(self.active['target'], rule)
        return result

    def run(self):
        self.start_time = time.monotonic()
        self.start_frame = self.client.state()['frame_count']
        reason = ''
        try:
            self.index = StoryIndex(self.client, self.maps_dir)
            self.record('index', rules=len(self.index.rules), errors=self.index.errors,
                        sha256=self.index.sha256, initial_facts=self.facts())
            if not self.index.rules:
                raise StoryStopped('no_planning_rules:rebuild_debug_binary')
            self.settle('begin exploring')
            while True:
                self.check_budget()
                facts = self.facts()
                self.mark_milestones(facts)
                if len(self.completed) == len(self.objectives):
                    break
                if (self.active is None or self.index.satisfied(self.active['target'], facts)
                        or self.should_replan(facts)):
                    self.select_strategy(facts)
                candidates, bindings = self.action_candidates(facts)
                if not candidates:
                    self.active = None
                    self.select_strategy(facts)
                    candidates, bindings = self.action_candidates(facts)
                try:
                    selection = self.choose('action', {
                        'subgoal': self.active['target'], 'advances': self.active['objectives'],
                        'local_state': facts, 'recent_outcomes': self.recent[-3:]}, candidates,
                        'Which next operation makes progress toward the current subgoal? '
                        'Several operations may be needed to finish it. Select a live trigger, '
                        'travel step, recovery or training operation using its described effects. '
                        'Script effects describe what will happen when the intended interaction completes. '
                        'Travel or interaction can be interrupted by other trainers: if recent attempts '
                        'gained battle flags, retrying the intended NPC is valid progress.')
                except StoryStopped as error:
                    if self.action_rejected(facts, str(error)):
                        continue
                    raise
                operation, rule = bindings[selection]
                result = self.execute(operation, rule)
                after = self.facts()
                changed = progress_key(facts) != progress_key(after)
                moved = tuple(facts[k] for k in ('map', 'x', 'y')) != tuple(after[k] for k in ('map', 'x', 'y'))
                delta = {'operation': operation, 'result': result.get('result'),
                         'flags_gained': sorted(k for k,v in after['flags'].items() if v and not facts['flags'].get(k)),
                         'bag_after': after['bag'], 'map': after['map'], 'story_state_changed': changed}
                delta['intended_effect_observed'] = self.index.satisfied(rule.effect, after)
                self.recent.append(delta)
                self.record('outcome', **delta)
                blocked = result.get('result') == 'blocked'
                if blocked:
                    # Reaching a story barrier is new planning evidence even
                    # when the player walked there. Offer its prerequisites now.
                    self.active = None
                if not changed and (not moved or blocked):
                    self.failures[attempt_key(rule, facts)] += 1
                    if self.failures[attempt_key(rule, facts)] >= 2:
                        self.active = None
        except StoryStopped as e:
            reason = str(e)
            self.record('stopped', reason=reason)
        facts = self.facts()
        self.mark_milestones(facts)
        success = all(self.objective_satisfied(o, facts) for o in self.objectives)
        return {'success': success, 'reason': '' if success else reason,
                'completed': self.completed, 'actions': self.actions,
                'calls': dict(self.calls), 'tokens': dict(self.tokens), 'models': sorted(self.models),
                'resolved_script_battles': self.resolved_battles,
                'travel_battles': self.travel_battles,
                'frames': self.client.state()['frame_count'] - self.start_frame,
                'wall_s': round(time.monotonic() - self.start_time, 3),
                'index_sha256': self.index.sha256 if self.index else None,
                'final_facts': facts}
