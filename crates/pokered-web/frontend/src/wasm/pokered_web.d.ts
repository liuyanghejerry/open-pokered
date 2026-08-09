declare module '@wasm/pokered_web.js' {
  export default function init(input?: RequestInfo | URL | BufferSource | WebAssembly.Module): Promise<WebAssembly.Instance>
}
