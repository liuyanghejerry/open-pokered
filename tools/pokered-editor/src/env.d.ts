declare module 'json-schema' {
  export type JSONSchema7 = Record<string, unknown>
}

declare module 'pokered-ui-preview' {
  export function render_layout(menu_name: string, layout_json: string, mock_state_id: number): Uint8Array
  export default function __wbg_init(): Promise<void>
}
