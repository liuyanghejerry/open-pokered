import { describe, expect, it } from 'vitest'
import { renderMarkdown } from './useMarkdown'

describe('renderMarkdown', () => {
  it('renders headings, bold, italic, and inline code', () => {
    expect(renderMarkdown('# Title')).toBe('<h1>Title</h1>')
    expect(renderMarkdown('a **b** and *c* and `d`')).toBe('<p>a <strong>b</strong> and <em>c</em> and <code>d</code></p>')
  })

  it('renders unordered and ordered lists', () => {
    expect(renderMarkdown('- one\n- two')).toBe('<ul>\n<li>one</li>\n<li>two</li>\n</ul>')
    expect(renderMarkdown('1. a\n2. b')).toBe('<ol>\n<li>a</li>\n<li>b</li>\n</ol>')
  })

  it('renders fenced code blocks without inner formatting and escapes them', () => {
    const html = renderMarkdown('```js\nconst x = a < b && **z**\n```')
    expect(html).toBe('<pre><code class="language-js">const x = a &lt; b &amp;&amp; **z**</code></pre>')
  })

  it('escapes raw HTML so it cannot inject markup (XSS-safe)', () => {
    const html = renderMarkdown('hello <img src=x onerror=alert(1)> world')
    expect(html).not.toContain('<img')
    expect(html).toContain('&lt;img')
  })

  it('sanitizes link hrefs and renders safe links', () => {
    expect(renderMarkdown('[ok](https://example.com)')).toContain('<a href="https://example.com"')
    // javascript: urls are rejected — only the label survives, no anchor
    const bad = renderMarkdown('[x](javascript:alert(1))')
    expect(bad).not.toContain('<a ')
    expect(bad).toContain('x')
  })

  it('separates paragraphs and keeps single newlines as <br>', () => {
    expect(renderMarkdown('a\nb\n\nc')).toBe('<p>a<br>b</p>\n<p>c</p>')
  })

  it('returns empty string for empty input', () => {
    expect(renderMarkdown('')).toBe('')
  })
})
