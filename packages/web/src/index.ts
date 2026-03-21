/**
 * workflow-graph — Interactive workflow DAG visualizer
 *
 * TypeScript wrapper around the WASM module.
 */

export interface Workflow {
  id: string;
  name: string;
  trigger: string;
  jobs: Job[];
}

export interface Job {
  id: string;
  name: string;
  status: 'queued' | 'running' | 'success' | 'failure' | 'skipped' | 'cancelled';
  command: string;
  duration_secs?: number;
  started_at?: number;
  depends_on: string[];
  output?: string;
  required_labels?: string[];
  max_retries?: number;
  attempt?: number;
  /** Arbitrary metadata for custom renderers (e.g., node_type, icon, color). */
  metadata?: Record<string, unknown>;
  /** Input and output ports for node-graph-style connections. */
  ports?: Port[];
}

/** Direction of a port on a node. */
export type PortDirection = 'input' | 'output';

/** A typed input or output port on a node. */
export interface Port {
  /** Unique identifier within the node (e.g., "message", "response"). */
  id: string;
  /** Display label. */
  label: string;
  /** Whether this is an input or output port. */
  direction: PortDirection;
  /** Type tag for connection compatibility (e.g., "text", "json", "tool_call"). */
  port_type?: string;
  /** Optional color override for the port dot. */
  color?: string;
}

/** An edge between two nodes (optionally port-to-port) with optional metadata. */
export interface EdgeInfo {
  from_id: string;
  to_id: string;
  /** Source port id (empty string = legacy node-to-node edge). */
  from_port?: string;
  /** Target port id (empty string = legacy node-to-node edge). */
  to_port?: string;
  metadata?: Record<string, unknown>;
}

// ─── Theme types ─────────────────────────────────────────────────────────────

export interface ThemeColors {
  success?: string;
  failure?: string;
  running?: string;
  queued?: string;
  skipped?: string;
  cancelled?: string;
  node_bg?: string;
  node_border?: string;
  text?: string;
  text_secondary?: string;
  bg?: string;
  graph_bg?: string;
  edge?: string;
  junction?: string;
  highlight?: string;
  selected?: string;
  header_text?: string;
  header_trigger?: string;
}

export interface ThemeFonts {
  family?: string;
  size_name?: number;
  size_duration?: number;
  size_header?: number;
}

export interface ThemeLayout {
  node_width?: number;
  node_height?: number;
  node_radius?: number;
  h_gap?: number;
  v_gap?: number;
  header_height?: number;
  padding?: number;
  junction_dot_radius?: number;
  status_icon_radius?: number;
  status_icon_margin?: number;
}

export type LayoutDirection = 'LeftToRight' | 'TopToBottom';

/** Per-edge style override. */
export interface EdgeStyle {
  /** CSS color for this edge (overrides theme default). */
  color?: string;
  /** Line width in px. */
  width?: number;
  /** Dash pattern array (e.g., [5, 3] for dashed). Empty = solid. */
  dash?: number[];
}

/** Internationalization labels for status text and duration formatting. */
export interface Labels {
  queued?: string;
  running?: string;
  success?: string;
  failure?: string;
  skipped?: string;
  cancelled?: string;
  /** Duration format for minutes+seconds. Use {m} and {s} placeholders. Default: "{m}m {s}s" */
  duration_minutes?: string;
  /** Duration format for seconds only. Use {s} placeholder. Default: "{s}s" */
  duration_seconds?: string;
}

/** Theme configuration. All fields are optional — omitted fields use light-theme defaults. */
export interface ThemeConfig {
  colors?: ThemeColors;
  fonts?: ThemeFonts;
  layout?: ThemeLayout;
  direction?: LayoutDirection;
  labels?: Labels;
  /** Per-edge style overrides keyed by "from_id->to_id". */
  edge_styles?: Record<string, EdgeStyle>;
  /** Show the minimap overlay. Default: false. */
  minimap?: boolean;
}

/**
 * Custom node render callback.
 * Called for each node during rendering.
 * @param x - Node x position
 * @param y - Node y position
 * @param w - Node width
 * @param h - Node height
 * @param job - The job data object
 * @returns `true` to skip default node rendering, `false` to render default on top.
 */
export type OnRenderNode = (
  x: number,
  y: number,
  w: number,
  h: number,
  job: Job,
) => boolean;

export interface GraphOptions {
  onNodeClick?: (jobId: string) => void;
  onNodeHover?: (jobId: string | null) => void;
  onCanvasClick?: () => void;
  onSelectionChange?: (selectedIds: string[]) => void;
  onNodeDragEnd?: (jobId: string, x: number, y: number) => void;
  /** Called when an edge is clicked. Requires bezier hit testing. */
  onEdgeClick?: (fromId: string, toId: string) => void;
  /** Custom node rendering callback. */
  onRenderNode?: OnRenderNode;
  /** Called when an external element is dropped on the canvas. x/y are graph-space coords. */
  onDrop?: (x: number, y: number, data: string) => void;
  /** Called when user drags from an output port to an input port to create a connection. */
  onConnect?: (fromNodeId: string, fromPortId: string, toNodeId: string, toPortId: string) => void;
  /** Custom theme configuration. */
  theme?: ThemeConfig;
  /** Automatically resize the canvas when the container resizes. */
  autoResize?: boolean;
}

/** WASM module interface — typed subset of exported functions. */
interface WasmModule {
  default(moduleOrPath?: string | URL | Request | RequestInfo): Promise<void>;
  render_workflow(
    canvasId: string, json: string,
    onNodeClick: ((jobId: string) => void) | null,
    onNodeHover: ((jobId: string | null) => void) | null,
    onCanvasClick: (() => void) | null,
    onSelectionChange: ((ids: string[]) => void) | null,
    onNodeDragEnd: ((jobId: string, x: number, y: number) => void) | null,
    themeJson: string | null,
  ): void;
  update_workflow_data(canvasId: string, json: string): void;
  set_theme(canvasId: string, json: string): void;
  set_auto_resize(canvasId: string, enabled: boolean): void;
  set_on_edge_click(canvasId: string, cb: (fromId: string, toId: string) => void): void;
  set_on_render_node(canvasId: string, cb: OnRenderNode): void;
  set_on_drop(canvasId: string, cb: (x: number, y: number, data: string) => void): void;
  set_on_connect(canvasId: string, cb: (fromNodeId: string, fromPortId: string, toNodeId: string, toPortId: string) => void): void;
  select_node(canvasId: string, jobId: string): void;
  deselect_all(canvasId: string): void;
  reset_layout(canvasId: string): void;
  zoom_to_fit(canvasId: string): void;
  set_zoom(canvasId: string, level: number): void;
  get_node_positions(canvasId: string): Record<string, [number, number]>;
  set_node_positions(canvasId: string, json: string): void;
  add_node(canvasId: string, jobJson: string): void;
  remove_node(canvasId: string, jobId: string): void;
  update_node(canvasId: string, jobId: string, partialJson: string): void;
  add_edge(canvasId: string, fromId: string, toId: string, fromPort: string | null, toPort: string | null, metadataJson: string | null): void;
  remove_edge(canvasId: string, fromId: string, toId: string): void;
  get_nodes(canvasId: string): Job[];
  get_edges(canvasId: string): EdgeInfo[];
  destroy(canvasId: string): void;
}

let wasmModule: WasmModule | null = null;
let customWasmUrl: string | URL | undefined;

/**
 * Configure the URL to the WASM binary. Call this before creating any WorkflowGraph instances.
 * Only needed if the default resolution doesn't work in your environment.
 *
 * @example
 * ```typescript
 * setWasmUrl('/assets/workflow_graph_web_bg.wasm');
 * // or from a CDN:
 * setWasmUrl('https://cdn.example.com/wasm/workflow_graph_web_bg.wasm');
 * ```
 */
export function setWasmUrl(url: string | URL): void {
  customWasmUrl = url;
}

async function ensureWasm(): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  // Dynamic import of the WASM glue code bundled in wasm/
  // @ts-expect-error — external wasm-pack artifact, not a TS module
  const mod: WasmModule = await import('../wasm/workflow_graph_web.js');
  await mod.default(customWasmUrl);
  wasmModule = mod;
  return wasmModule;
}

/**
 * Interactive workflow DAG graph component.
 *
 * @example
 * ```typescript
 * const graph = new WorkflowGraph(document.getElementById('container')!, {
 *   onNodeClick: (jobId) => console.log('clicked', jobId),
 *   theme: darkTheme,
 *   autoResize: true,
 * });
 * graph.setWorkflow(workflowData);
 * ```
 */
export class WorkflowGraph {
  private canvasId: string;
  private canvas: HTMLCanvasElement;
  private options: GraphOptions;
  private initialized = false;

  constructor(container: HTMLElement, options: GraphOptions = {}) {
    this.canvasId = `gg-${Math.random().toString(36).slice(2, 9)}`;
    this.canvas = document.createElement('canvas');
    this.canvas.id = this.canvasId;
    this.canvas.style.display = 'block';
    this.canvas.setAttribute('role', 'img');
    this.canvas.setAttribute('aria-label', 'Workflow DAG visualization');
    this.canvas.tabIndex = 0;
    container.appendChild(this.canvas);
    this.options = options;
  }

  /** Render a workflow. Call this on initial load. */
  async setWorkflow(workflow: Workflow): Promise<void> {
    const wasm = await ensureWasm();
    const json = JSON.stringify(workflow);
    const themeJson = this.options.theme ? JSON.stringify(this.options.theme) : null;
    wasm.render_workflow(
      this.canvasId,
      json,
      this.options.onNodeClick || null,
      this.options.onNodeHover || null,
      this.options.onCanvasClick || null,
      this.options.onSelectionChange || null,
      this.options.onNodeDragEnd || null,
      themeJson,
    );
    this.initialized = true;

    // Wire up optional callbacks that use separate WASM functions
    if (this.options.onEdgeClick) {
      wasm.set_on_edge_click(this.canvasId, this.options.onEdgeClick);
    }
    if (this.options.onRenderNode) {
      wasm.set_on_render_node(this.canvasId, this.options.onRenderNode);
    }
    if (this.options.onDrop) {
      wasm.set_on_drop(this.canvasId, this.options.onDrop);
    }
    if (this.options.onConnect) {
      wasm.set_on_connect(this.canvasId, this.options.onConnect);
    }
    if (this.options.autoResize) {
      wasm.set_auto_resize(this.canvasId, true);
    }
  }

  /** Update workflow data without resetting positions or zoom. */
  async updateStatus(workflow: Workflow): Promise<void> {
    const wasm = await ensureWasm();
    if (this.initialized) {
      wasm.update_workflow_data(this.canvasId, JSON.stringify(workflow));
    } else {
      await this.setWorkflow(workflow);
    }
  }

  /** Update the theme at runtime. Triggers re-layout if dimensions or direction changed. */
  async setTheme(theme: ThemeConfig): Promise<void> {
    const wasm = await ensureWasm();
    wasm.set_theme(this.canvasId, JSON.stringify(theme));
    this.options.theme = theme;
  }

  /** Enable or disable auto-resize via ResizeObserver. */
  async setAutoResize(enabled: boolean): Promise<void> {
    const wasm = await ensureWasm();
    wasm.set_auto_resize(this.canvasId, enabled);
    this.options.autoResize = enabled;
  }

  /** Programmatically select a node. */
  async selectNode(jobId: string): Promise<void> {
    const wasm = await ensureWasm();
    wasm.select_node(this.canvasId, jobId);
  }

  /** Deselect all nodes. */
  async deselectAll(): Promise<void> {
    const wasm = await ensureWasm();
    wasm.deselect_all(this.canvasId);
  }

  /** Reset node positions to auto-layout. */
  async resetLayout(): Promise<void> {
    const wasm = await ensureWasm();
    wasm.reset_layout(this.canvasId);
  }

  /** Zoom to fit the entire graph. */
  async zoomToFit(): Promise<void> {
    const wasm = await ensureWasm();
    wasm.zoom_to_fit(this.canvasId);
  }

  /** Set zoom level (0.25 to 4.0). */
  async setZoom(level: number): Promise<void> {
    const wasm = await ensureWasm();
    wasm.set_zoom(this.canvasId, level);
  }

  /** Get current node positions for persistence. */
  async getNodePositions(): Promise<Record<string, [number, number]>> {
    const wasm = await ensureWasm();
    return wasm.get_node_positions(this.canvasId);
  }

  /** Restore previously saved node positions. */
  async setNodePositions(positions: Record<string, [number, number]>): Promise<void> {
    const wasm = await ensureWasm();
    wasm.set_node_positions(this.canvasId, JSON.stringify(positions));
  }

  // ─── Node CRUD API ─────────────────────────────────────────────────────────

  /** Add a new node to the graph. Triggers re-layout. */
  async addNode(job: Job): Promise<void> {
    const wasm = await ensureWasm();
    wasm.add_node(this.canvasId, JSON.stringify(job));
  }

  /** Remove a node and all its connected edges. Triggers re-layout. */
  async removeNode(jobId: string): Promise<void> {
    const wasm = await ensureWasm();
    wasm.remove_node(this.canvasId, jobId);
  }

  /** Update a node's properties via partial JSON merge. */
  async updateNode(jobId: string, partial: Partial<Job>): Promise<void> {
    const wasm = await ensureWasm();
    wasm.update_node(this.canvasId, jobId, JSON.stringify(partial));
  }

  /** Add an edge between two nodes, optionally specifying ports. */
  async addEdge(fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>): Promise<void> {
    const wasm = await ensureWasm();
    wasm.add_edge(this.canvasId, fromId, toId, fromPort ?? null, toPort ?? null, metadata ? JSON.stringify(metadata) : null);
  }

  /** Remove an edge between two nodes. */
  async removeEdge(fromId: string, toId: string): Promise<void> {
    const wasm = await ensureWasm();
    wasm.remove_edge(this.canvasId, fromId, toId);
  }

  /** Get all nodes in the graph. */
  async getNodes(): Promise<Job[]> {
    const wasm = await ensureWasm();
    return wasm.get_nodes(this.canvasId);
  }

  /** Get all edges in the graph. */
  async getEdges(): Promise<EdgeInfo[]> {
    const wasm = await ensureWasm();
    return wasm.get_edges(this.canvasId);
  }

  /** Clean up event listeners, resize observer, and remove the canvas. */
  async destroy(): Promise<void> {
    const wasm = await ensureWasm();
    wasm.destroy(this.canvasId);
    this.canvas.remove();
    this.initialized = false;
  }
}

// ─── Preset themes ───────────────────────────────────────────────────────────

/** GitHub Actions dark mode theme preset. */
export const darkTheme: ThemeConfig = {
  colors: {
    success: '#3fb950',
    failure: '#f85149',
    running: '#d29922',
    queued: '#8b949e',
    skipped: '#8b949e',
    cancelled: '#8b949e',
    node_bg: '#161b22',
    node_border: '#30363d',
    text: '#e6edf3',
    text_secondary: '#8b949e',
    bg: '#0d1117',
    graph_bg: '#161b22',
    edge: '#30363d',
    junction: '#484f58',
    highlight: '#58a6ff',
    selected: '#58a6ff',
    header_text: '#e6edf3',
    header_trigger: '#8b949e',
  },
};

/** Default GitHub Actions light theme (no-op, but useful for toggling back). */
export const lightTheme: ThemeConfig = {};

/** WCAG AA high-contrast theme for accessibility. */
export const highContrastTheme: ThemeConfig = {
  colors: {
    success: '#008000',
    failure: '#ff0000',
    running: '#ff8c00',
    queued: '#555555',
    skipped: '#555555',
    cancelled: '#555555',
    node_bg: '#ffffff',
    node_border: '#000000',
    text: '#000000',
    text_secondary: '#333333',
    bg: '#ffffff',
    graph_bg: '#f0f0f0',
    edge: '#000000',
    junction: '#000000',
    highlight: '#0000ff',
    selected: '#0000ff',
    header_text: '#000000',
    header_trigger: '#333333',
  },
};
