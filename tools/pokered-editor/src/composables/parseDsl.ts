export interface DslBlock {
  type: 'storyline' | 'speaker' | 'choice' | 'run' | 'load' | 'if' | 'each' | 'variables' | 'theme' | 'style' | 'atlas' | 'option' | 'command' | 'trigger'
  name?: string
  line: number
}

const DIRECTIVE_RE = /@(storyline|speaker|choice|run|load|if|each|variables|theme|style|atlas|option|command|trigger)\b/

export function parseDsl(content: string): DslBlock[] {
  const result: DslBlock[] = []
  const lines = content.split('\n')
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(DIRECTIVE_RE)
    if (m) {
      const type = m[1] as DslBlock['type']
      // Extract name from parentheses with optional quotes
      let name: string | undefined
      const nameMatch = lines[i].match(/\(["']([^"']+)["']\)/)
      if (nameMatch) {
        name = nameMatch[1]
      }
      result.push({ type, name, line: i + 1 })
    }
  }
  return result
}
