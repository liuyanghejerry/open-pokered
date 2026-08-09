<script setup lang="ts">
// Collapsible quick-reference for the *implemented* GUI (.gui) DSL.
// Source of truth: docs/GAME_UI_DSL.md (implemented subset only — the proposal
// features like @if/@each, @theme, flexbox and animations are NOT compiled by
// the .gui pipeline and are intentionally omitted here). Bilingual text via
// @t("en", "中文") IS compiled (see the i18n section).
import { ref } from 'vue'

const open = ref(false)

interface Component {
  name: string
  desc: string
  example: string
}

// One row per implemented component type.
const components: Component[] = [
  { name: 'panel', desc: 'Bordered box (Game Boy text frame).', example: 'panel {\n  rect = {tx: 0, ty: 12, tw: 20, th: 6}\n  style = "default"   // default | single | double\n}' },
  { name: 'container', desc: 'Invisible group; can be toggled with visible.', example: 'container {\n  rect = {tx: 0, ty: 0, tw: 20, th: 18}\n  visible = "{show_entry1}"\n  text("{mon1_name}") { rect = {tx: 4, ty: 0, tw: 10, th: 1} }\n}' },
  { name: 'text', desc: 'A line of text. The arg is the value (or use value = "…").', example: 'text("PLAYER") {\n  rect = {tx: 5, ty: 2, tw: 7, th: 1}\n  color = "Black"     // Black|DarkGray|LightGray|White|#rrggbb\n  align = "left"      // left | center | right\n  wrap = "word"       // word-wrap inside the rect\n  line_spacing = 1\n}\n\ntext("{player_name}") { rect = {tx: 13, ty: 2, tw: 5, th: 1} }' },
  { name: 'tile', desc: 'Draw a single tile by id (cursor arrows, sprites…).', example: 'tile(223) { rect = {tx: 14, ty: 7, tw: 1, th: 1} }\n\ntile("{sprite_index}") {\n  rect = {tx: 15, ty: 4, tw: 2, th: 2}\n  visible = "{has_selected}"\n  flip_x = false   flip_y = false\n}' },
  { name: 'divider', desc: 'Repeated tile run (a separator line).', example: 'divider {\n  rect = {tx: 1, ty: 9, tw: 18, th: 1}\n  tiles = [122]\n  repeat = 17\n  orientation = "horizontal"   // horizontal | vertical\n}' },
  { name: 'list', desc: 'Single-column scrolling list bound to a {var} array.', example: 'list {\n  rect = {tx: 11, ty: 1, tw: 8, th: 13}\n  source = "{items}"\n  item_template = {height: 1, gap: 1}\n  cursor = {tile: 223, position: "left"}\n  max_visible = 7\n}' },
  { name: 'flex_list', desc: 'Multi-column list; the arg is the data binding.', example: 'flex_list("{bag_items}") {\n  rect = {tx: 1, ty: 4, tw: 18, th: 13}\n  item_layout = [\n    {field: "name", width: 14, align: "left"},\n    {field: "qty",  width: 3,  align: "right", prefix: "x"}\n  ]\n  padding = {top: 1, left: 1}\n  gap = 1\n  cursor = {tile: 223, position: "left"}\n}' },
  { name: 'button', desc: 'Interactive text with a click handler.', example: 'button("OK") {\n  rect = {tx: 10, ty: 15, tw: 5, th: 1}\n  on_click = "handler"\n}' },
  { name: 'image', desc: 'An image, optional nine-slice.', example: 'image("sprite.png") {\n  rect = {tx: 0, ty: 0, tw: 7, th: 7}\n  slice = "[8,8,8,8]"\n}' },
  { name: 'input / dropdown', desc: 'Form field stubs (rarely used in pokered screens).', example: 'input { rect = {tx: 0, ty: 0, tw: 20, th: 1} }\ndropdown { rect = {tx: 0, ty: 0, tw: 10, th: 1} }' },
]

const i18nExample = `text(@t("TEXT SPEED", "文字速度")) {
  rect = {tx: 1, ty: 1, tw: 16, th: 1}
}
button(@t("CANCEL", "取消")) {
  rect = {tx: 2, ty: 16, tw: 8, th: 1}
}`

const dialogExample = `screen Dialog {
  panel {
    rect = {tx: 0, ty: 12, tw: 20, th: 6}
    style = "default"
    text("{text}") {
      rect = {tx: 1, ty: 13, tw: 18, th: 4}
      wrap = "word"
      line_spacing = 1
    }
    tile(31) { rect = {tx: 18, ty: 16, tw: 1, th: 1} }
  }
}`
</script>

<template>
  <details
    class="mt-2 rounded border border-[rgba(255,255,255,0.08)] bg-bg-inset/30 text-[11px]"
    :open="open"
    @toggle="open = ($event.target as HTMLDetailsElement).open"
  >
    <summary
      class="cursor-pointer select-none px-2 py-1.5 flex items-center gap-2 text-text-muted hover:text-text"
    >
      <span class="transition-transform" :class="open ? 'rotate-90' : ''">▶</span>
      <span class="font-bold uppercase tracking-wider text-[10px]">📖 GUI DSL Reference</span>
      <span class="text-[10px] text-text-muted/70">syntax for .gui layouts</span>
    </summary>

    <div class="max-h-[42vh] overflow-y-auto px-3 pb-3 pt-1 space-y-3 leading-relaxed">
      <!-- Basics -->
      <section>
        <h4 class="text-accent font-bold mb-1">Structure</h4>
        <p class="text-text-muted">
          Wrap the layout in <code class="text-text">screen Name { … }</code>. Components nest with
          <code class="text-text">{ }</code>; some take a positional argument
          (<code class="text-text">text("Hi")</code>, <code class="text-text">tile(31)</code>,
          <code class="text-text">flex_list("{items}")</code>). Line comments use
          <code class="text-text">//</code>.
        </p>
      </section>

      <!-- Coordinates -->
      <section>
        <h4 class="text-accent font-bold mb-1">Coordinates — rect</h4>
        <p class="text-text-muted">
          The screen is a <strong class="text-text">20×18 tile</strong> grid. Position elements
          absolutely with <code class="text-text">rect = {tx, ty, tw, th}</code>
          (column, row, width, height in tiles). Coords accept bindings:
          <code class="text-text">tx: "{cursor_x}"</code>.
        </p>
      </section>

      <!-- Data binding -->
      <section>
        <h4 class="text-accent font-bold mb-1">Data binding</h4>
        <p class="text-text-muted">
          A string is drawn literally; <code class="text-text">"{var}"</code> is resolved from the
          render data at runtime (see the Variables panel). Examples:
          <code class="text-text">"{player_name}"</code>,
          <code class="text-text">"MONEY ${balance}"</code>,
          <code class="text-text">"{owned_count}/151"</code>.
        </p>
      </section>

      <!-- i18n -->
      <section>
        <h4 class="text-accent font-bold mb-1">Bilingual text — @t</h4>
        <p class="text-text-muted">
          Wrap any <code class="text-text">text(…)</code> / <code class="text-text">button(…)</code>
          label in <code class="text-text">@t("English", "中文")</code> to make it bilingual
          (English first, Chinese second). It compiles to a
          <code class="text-text">{ "en": …, "zh": … }</code> value; the preview shows the
          variant for the selected language. Plain strings are unchanged.
        </p>
        <pre v-text="i18nExample" class="mt-0.5 p-2 rounded bg-bg-inset font-mono text-[10px] text-text overflow-x-auto whitespace-pre" />
        <p class="text-text-muted/80 mt-1">
          Mixed static + binding works too:
          <code class="text-text">@t("MONEY ${balance}", "金钱 ${balance}")</code>.
        </p>
      </section>

      <!-- Components -->
      <section>
        <h4 class="text-accent font-bold mb-1">Components</h4>
        <div class="space-y-2">
          <div v-for="c in components" :key="c.name">
            <div class="flex items-baseline gap-2">
              <code class="text-info font-bold">{{ c.name }}</code>
              <span class="text-text-muted">{{ c.desc }}</span>
            </div>
            <pre v-text="c.example" class="mt-0.5 p-2 rounded bg-bg-inset font-mono text-[10px] text-text overflow-x-auto whitespace-pre" />
          </div>
        </div>
      </section>

      <!-- Common props -->
      <section>
        <h4 class="text-accent font-bold mb-1">Common properties</h4>
        <ul class="text-text-muted list-disc pl-4 space-y-0.5">
          <li><code class="text-text">rect</code> — position/size (all components)</li>
          <li><code class="text-text">value</code> — text body (alias for the positional arg)</li>
          <li><code class="text-text">color</code> — Black · DarkGray · LightGray · White · #rrggbb</li>
          <li><code class="text-text">align</code> — left · center · right &nbsp;|&nbsp; <code class="text-text">font</code>, <code class="text-text">wrap = "word"</code>, <code class="text-text">line_spacing</code></li>
          <li><code class="text-text">style</code> — panel border: default · single · double</li>
          <li><code class="text-text">visible</code> — <code class="text-text">"{flag}"</code> conditional render</li>
          <li><code class="text-text">source</code> / <code class="text-text">item_template</code> / <code class="text-text">max_visible</code> — list data &amp; rows</li>
          <li><code class="text-text">item_layout</code> — flex_list columns: <code class="text-text">{field, width, align, prefix}</code></li>
          <li><code class="text-text">cursor</code> — <code class="text-text">{tile: 223, position: "left"}</code></li>
          <li><code class="text-text">tile_id</code>, <code class="text-text">tiles</code>, <code class="text-text">repeat</code>, <code class="text-text">orientation</code>, <code class="text-text">flip_x/flip_y</code>, <code class="text-text">palette</code></li>
        </ul>
      </section>

      <!-- Example -->
      <section>
        <h4 class="text-accent font-bold mb-1">Example — dialog box</h4>
        <pre v-text="dialogExample" class="p-2 rounded bg-bg-inset font-mono text-[10px] text-text overflow-x-auto whitespace-pre" />
      </section>

      <p class="text-text-muted/70 text-[10px] pt-1 border-t border-[rgba(255,255,255,0.06)]">
        Note: <code>@if</code>/<code>@each</code>, <code>@theme</code>,
        flexbox and animations are proposals and are <strong>not</strong> compiled by .gui.
        (<code>@t</code> bilingual text <strong>is</strong> compiled — see above.)
        Full reference: <code>docs/GAME_UI_DSL.md</code>.
      </p>
    </div>
  </details>
</template>
