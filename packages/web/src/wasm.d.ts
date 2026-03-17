// Type declarations for the WASM glue module
declare module '../wasm/workflow_graph_web.js' {
  const init: (moduleOrPath?: string | URL | Request | RequestInfo) => Promise<void>;
  export default init;
  export function render_workflow(...args: unknown[]): void;
  export function update_workflow_data(...args: unknown[]): void;
  export function set_theme(...args: unknown[]): void;
  export function set_auto_resize(...args: unknown[]): void;
  export function set_on_edge_click(...args: unknown[]): void;
  export function set_on_render_node(...args: unknown[]): void;
  export function select_node(...args: unknown[]): void;
  export function deselect_all(...args: unknown[]): void;
  export function reset_layout(...args: unknown[]): void;
  export function zoom_to_fit(...args: unknown[]): void;
  export function set_zoom(...args: unknown[]): void;
  export function get_node_positions(...args: unknown[]): unknown;
  export function set_node_positions(...args: unknown[]): void;
  export function destroy(...args: unknown[]): void;
}
