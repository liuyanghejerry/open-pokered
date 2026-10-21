"""The driver must respect source menu publication before spending key edges."""
import unittest
import playthrough_late as late


class MenuGame:
    def __init__(self, cursor=1, ready_after=5, consume_start=False, wrong_menu=False):
        self.cursor = cursor
        self.ready_after = ready_after
        self.consume_start = consume_start
        self.wrong_menu = wrong_menu
        self.opened = False
        self.kind = 'start'
        self.frames = 0
        self.events = []
        self.items = ['Pokedex', 'Pokemon', 'Item']

    def st(self):
        if not self.opened:
            return {'screen': 'overworld', 'field_menu': None}
        return {'screen': 'start-menu' if self.kind == 'start' else 'party',
                'field_menu': {'kind': self.kind, 'cursor': self.cursor,
                               'items': self.items,
                               'ready_for_input': self.frames >= self.ready_after}}

    def step(self, frames):
        self.events.append(('step', frames))
        self.frames += frames
        if self.wrong_menu:
            self.kind = 'party'

    def tap(self, button, gap):
        self.events.append(('tap', button, gap))
        if button == 'start':
            if self.consume_start:
                self.consume_start = False
                return
            self.opened = True
            return
        assert self.frames >= self.ready_after, f'{button} sent before ready'
        if button == 'down':
            self.cursor = (self.cursor + 1) % len(self.items)
        elif button == 'a':
            self.kind = 'party'
        else:
            raise AssertionError(button)


class StartMenuReadinessTests(unittest.TestCase):
    def test_remembered_pokemon_cursor_waits_then_opens_party_with_one_confirm(self):
        game = MenuGame()
        late.open_start(game, 'Pokemon')
        self.assertEqual(game.st()['field_menu']['kind'], 'party')
        self.assertEqual(game.events, [('tap', 'start', 12)] + [('step', 1)] * 5 + [('tap', 'a', 12)])

    def test_cursor_navigation_also_waits_for_menu_publication(self):
        game = MenuGame(cursor=0)
        late.open_start(game, 'Pokemon')
        self.assertEqual(game.events[-2:], [('tap', 'down', 8), ('tap', 'a', 12)])
        self.assertEqual(game.frames, 5)

    def test_ready_menu_keeps_existing_input_sequence(self):
        game = MenuGame(ready_after=0)
        late.open_start(game, 'Pokemon')
        self.assertEqual(game.events, [('tap', 'start', 12), ('tap', 'a', 12)])

    def test_never_ready_is_bounded_and_sends_no_selection(self):
        game = MenuGame(ready_after=1000)
        with self.assertRaisesRegex(RuntimeError, 'did not become ready'):
            late.open_start(game, 'Pokemon')
        self.assertEqual(game.frames, 120)
        self.assertEqual([e for e in game.events if e[0] == 'tap'], [('tap', 'start', 12)])

    def test_unexpected_submenu_during_wait_remains_an_error(self):
        game = MenuGame(wrong_menu=True)
        with self.assertRaises(AssertionError):
            late.open_start(game, 'Pokemon')
        self.assertNotIn(('tap', 'a', 12), game.events)

    def test_final_warp_lock_can_still_consume_first_start(self):
        game = MenuGame(consume_start=True)
        late.open_start(game, 'Pokemon')
        self.assertEqual(sum(e[:2] == ('tap', 'start') for e in game.events), 2)
        self.assertEqual(game.st()['field_menu']['kind'], 'party')

    def test_missing_entry_retains_navigation_failure(self):
        game = MenuGame(ready_after=0)
        with self.assertRaisesRegex(RuntimeError, 'entry not found'):
            late.open_start(game, 'DoesNotExist')
        self.assertEqual(sum(e[:2] == ('tap', 'down') for e in game.events), 20)
        self.assertNotIn(('tap', 'a', 12), game.events)


if __name__ == '__main__':
    unittest.main()
