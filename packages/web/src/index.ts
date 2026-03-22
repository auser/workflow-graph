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
  /** Child nodes for compound (grouped) nodes. */
  children?: Job[];
  /** Whether this compound node is collapsed (renders as single node). */
  collapsed?: boolean;
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

// ─── Node Definition types ──────────────────────────────────────────────────

/** Type of inline field rendered inside a node body. */
export type FieldType = 'text' | 'textarea' | 'select' | 'toggle' | 'badge' | 'slider';

/** Definition of an inline field rendered inside a node. */
export interface FieldDef {
  /** Key used to read/write the field value in `Job.metadata`. */
  key: string;
  /** What kind of control to render. */
  type: FieldType;
  /** Display label. */
  label: string;
  /** Available options (for `select` fields). */
  options?: string[];
  /** Default value. */
  defaultValue?: unknown;
  /** Minimum value (for `slider` fields). */
  min?: number;
  /** Maximum value (for `slider` fields). */
  max?: number;
}

/**
 * Declarative definition of a node type.
 *
 * Registered via `WorkflowGraph.registerNodeType()`. The renderer uses this
 * to draw colored headers, inline fields, and type-specific visuals.
 * Consumers can define any number of custom node types.
 */
export interface NodeDefinition {
  /** Unique type key (e.g., "agent", "tool"). Matched against `Job.metadata.node_type`. */
  type: string;
  /** Display label shown in the header bar. */
  label: string;
  /** Icon character (emoji or Unicode) rendered in the header. */
  icon?: string;
  /** Hex color for the header bar (e.g., "#3b82f6"). */
  headerColor?: string;
  /** Category for grouping in palettes (consumer-defined, no constraints). */
  category?: string;
  /** Inline fields rendered in the node body. */
  fields?: FieldDef[];
  /** Default input ports for this node type. */
  inputs?: Port[];
  /** Default output ports for this node type. */
  outputs?: Port[];
}

/** Serializable graph state for persistence. */
export interface GraphState {
  version: number;
  workflow: Workflow;
  positions: Record<string, [number, number]>;
  edges: EdgeInfo[];
  zoom: number;
  pan_x: number;
  pan_y: number;
}

/** Storage backend for auto-persistence. */
export interface PersistStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

/** Auto-persistence configuration. */
export interface PersistOptions {
  /** Storage key name. Default: 'workflow-graph-state' */
  key?: string;
  /** Storage backend. Default: localStorage */
  storage?: PersistStorage;
}

/**
 * Callback for DOM-based node rendering.
 * Called for each node — return an HTMLElement to render it as a DOM overlay
 * instead of (or on top of) canvas rendering. Return null to use default canvas rendering.
 * The library handles positioning, scaling, and z-ordering.
 */
export type NodeRenderer = (
  node: Job,
  definition: NodeDefinition | null,
) => HTMLElement | null;

export interface GraphOptions {
  onNodeClick?: (jobId: string) => void;
  onNodeHover?: (jobId: string | null) => void;
  onCanvasClick?: () => void;
  onSelectionChange?: (selectedIds: string[]) => void;
  onNodeDragEnd?: (jobId: string, x: number, y: number) => void;
  /** Called when an edge is clicked. Requires bezier hit testing. */
  onEdgeClick?: (fromId: string, toId: string) => void;
  /** Custom canvas node rendering callback (legacy). */
  onRenderNode?: OnRenderNode;
  /**
   * DOM-based node rendering. Provide a function that returns an HTMLElement for each node.
   * The library positions and scales the element over the canvas. Return null for default rendering.
   * When set, canvas node body rendering is skipped for nodes that return an element
   * (edges and ports are still drawn on canvas).
   */
  nodeRenderer?: NodeRenderer;
  /** Called when an external element is dropped on the canvas. x/y are graph-space coords. */
  onDrop?: (x: number, y: number, data: string) => void;
  /** Called when user drags from an output port to an input port to create a connection. */
  onConnect?: (fromNodeId: string, fromPortId: string, toNodeId: string, toPortId: string) => void;
  /** Called when user clicks on an inline field within a node. screenX/screenY are viewport coordinates for overlay positioning. */
  onFieldClick?: (nodeId: string, fieldKey: string, screenX: number, screenY: number) => void;
  /** Custom theme configuration. */
  theme?: ThemeConfig;
  /** Automatically resize the canvas when the container resizes. */
  autoResize?: boolean;
  /** Enable auto-persistence. Graph state saved on every change. */
  persist?: PersistOptions | boolean;
  /** Initial node positions to restore after layout. */
  initialPositions?: Record<string, [number, number]>;
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
  redraw(canvasId: string): void;
  resize_canvas(canvasId: string, width: number, height: number): void;
  enable_auto_resize(canvasId: string): void;
  mark_destroyed(canvasId: string): void;
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
  add_node(canvasId: string, jobJson: string, x?: number, y?: number): void;
  remove_node(canvasId: string, jobId: string): void;
  update_node(canvasId: string, jobId: string, partialJson: string): void;
  add_edge(canvasId: string, fromId: string, toId: string, fromPort: string | null, toPort: string | null, metadataJson: string | null): void;
  remove_edge(canvasId: string, fromId: string, toId: string): void;
  get_nodes(canvasId: string): Job[];
  get_edges(canvasId: string): EdgeInfo[];
  get_state(canvasId: string): GraphState;
  load_state(canvasId: string, stateJson: string): void;
  group_selected(canvasId: string, groupName: string): void;
  ungroup_node(canvasId: string, nodeId: string): void;
  toggle_collapse(canvasId: string, nodeId: string): void;
  register_node_type(canvasId: string, defJson: string): void;
  set_on_field_click(canvasId: string, cb: (nodeId: string, fieldKey: string, screenX: number, screenY: number) => void): void;
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
  private container: HTMLElement;
  private options: GraphOptions;
  private initialized = false;
  private destroyed = false;
  private persistKey: string | null = null;
  private persistStorage: PersistStorage | null = null;
  private nodeTypeRegistry: Map<string, NodeDefinition> = new Map();
  private resizeObserver: ResizeObserver | null = null;
  private wasmRef: WasmModule | null = null;

  // DOM node rendering layer
  private nodeOverlayLayer: HTMLDivElement | null = null;
  private nodeOverlayElements: Map<string, HTMLDivElement> = new Map();
  private nodeOverlayRafId = 0;

  constructor(container: HTMLElement, options: GraphOptions = {}) {
    this.canvasId = `gg-${Math.random().toString(36).slice(2, 9)}`;
    this.container = container;

    // Ensure container has relative positioning for overlay layer
    const pos = getComputedStyle(container).position;
    if (pos === 'static') {
      container.style.position = 'relative';
    }

    this.canvas = document.createElement('canvas');
    this.canvas.id = this.canvasId;
    this.canvas.style.display = 'block';
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.setAttribute('role', 'img');
    this.canvas.setAttribute('aria-label', 'Workflow DAG visualization');
    this.canvas.tabIndex = 0;
    container.appendChild(this.canvas);
    this.options = options;

    // Create DOM overlay layer if nodeRenderer is provided
    if (options.nodeRenderer) {
      this._createOverlayLayer();
    }

    // Set up auto-persistence
    if (options.persist) {
      const persistOpts = typeof options.persist === 'boolean'
        ? {} : options.persist;
      this.persistKey = persistOpts.key ?? 'workflow-graph-state';
      this.persistStorage = persistOpts.storage ?? (typeof localStorage !== 'undefined' ? localStorage : null);
    }

    // Wrap onNodeDragEnd to auto-save
    const originalDragEnd = options.onNodeDragEnd;
    options.onNodeDragEnd = (jobId, x, y) => {
      originalDragEnd?.(jobId, x, y);
      this.autoPersist();
    };
  }

  /** Save state to configured storage */
  private autoPersist(): void {
    if (!this.persistKey || !this.persistStorage || this.destroyed) return;
    this.getState().then(state => {
      if (state && this.persistKey && this.persistStorage) {
        this.persistStorage.setItem(this.persistKey, JSON.stringify(state));
      }
    }).catch(() => {});
  }

  /** Restore full persisted state (workflow, positions, edges, zoom, pan). */
  async restorePersistedState(): Promise<boolean> {
    if (!this.persistKey || !this.persistStorage) return false;
    try {
      const raw = this.persistStorage.getItem(this.persistKey);
      if (!raw) return false;
      const state: GraphState = JSON.parse(raw);
      if (state) {
        await this.loadState(state);
        return true;
      }
    } catch {
      // Invalid stored state — ignore
    }
    return false;
  }

  /** Clear persisted state */
  clearPersistedState(): void {
    if (this.persistKey && this.persistStorage) {
      this.persistStorage.removeItem(this.persistKey);
    }
  }

  /** Render a workflow. Call this on initial load. */
  async setWorkflow(workflow: Workflow): Promise<void> {
    const wasm = await ensureWasm();
    this.wasmRef = wasm; // Cache for synchronous access in destroy()
    const json = JSON.stringify(workflow);
    const themeJson = this.options.theme ? JSON.stringify(this.options.theme) : null;
    try {
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
    } catch (e) {
      console.warn('workflow-graph: render_workflow failed, continuing anyway:', e);
    }
    // Always mark as initialized — the canvas and GRAPHS entry exist even if rendering had issues
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
      // Use WASM auto_resize — sets the flag AND creates an observer that sizes
      // the canvas to the parent on every resize. mark_destroyed() ensures the
      // observer callback bails out safely when the graph is destroyed.
      wasm.set_auto_resize(this.canvasId, true);
    }

    // Restore persisted positions after initial layout
    if (this.persistKey) {
      await this.restorePersistedState();
    }

    // Apply initial positions prop (from React)
    if (this.options.initialPositions && Object.keys(this.options.initialPositions).length > 0) {
      await this.setNodePositions(this.options.initialPositions).catch(() => {});
    }

    // Save initial state so it persists even without user interaction
    this.autoPersist();
  }

  /** Update workflow data without resetting positions or zoom. */
  async updateStatus(workflow: Workflow): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    if (this.initialized) {
      // Save positions before update (updateStatus may trigger layout changes)
      const rawPositions = wasm.get_node_positions(this.canvasId);
      const positions = typeof rawPositions === 'string' ? JSON.parse(rawPositions) : rawPositions;
      wasm.update_workflow_data(this.canvasId, JSON.stringify(workflow));
      // Restore positions after update
      if (positions && Object.keys(positions).length > 0) {
        wasm.set_node_positions(this.canvasId, JSON.stringify(positions));
      }
      // Don't autoPersist here — updateStatus runs every topology poll (3s)
      // and can overwrite user-dragged positions if WASM state was corrupted.
      // Persistence is handled by user actions (drag, add, remove, connect).
    } else {
      await this.setWorkflow(workflow);
    }
  }

  /** Update the theme at runtime. Triggers re-layout if dimensions or direction changed. */
  async setTheme(theme: ThemeConfig): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.set_theme(this.canvasId, JSON.stringify(theme));
    this.options.theme = theme;
  }

  /** Enable or disable auto-resize via ResizeObserver. */
  async setAutoResize(enabled: boolean): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.set_auto_resize(this.canvasId, enabled);
    this.options.autoResize = enabled;
  }

  /** Programmatically select a node. */
  async selectNode(jobId: string): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.select_node(this.canvasId, jobId);
  }

  /** Deselect all nodes. */
  async deselectAll(): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.deselect_all(this.canvasId);
  }

  /** Reset node positions to auto-layout. */
  async resetLayout(): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.reset_layout(this.canvasId);
  }

  /** Zoom to fit the entire graph. */
  async zoomToFit(): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.zoom_to_fit(this.canvasId);
  }

  /** Set zoom level (0.25 to 4.0). */
  async setZoom(level: number): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.set_zoom(this.canvasId, level);
  }

  /** Get current node positions for persistence. */
  async getNodePositions(): Promise<Record<string, [number, number]>> {
    if (!this.alive) return {};
    const wasm = await ensureWasm();
    const result = wasm.get_node_positions(this.canvasId);
    if (typeof result === 'string') {
      try { return JSON.parse(result); } catch { return {}; }
    }
    return result;
  }

  /** Restore previously saved node positions. */
  async setNodePositions(positions: Record<string, [number, number]>): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.set_node_positions(this.canvasId, JSON.stringify(positions));
  }

  // ─── Node CRUD API ─────────────────────────────────────────────────────────

  /** Add a new node to the graph. Optionally specify position (x, y) in graph-space. */
  async addNode(job: Job, x?: number, y?: number): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.add_node(this.canvasId, JSON.stringify(job), x, y);
    this.autoPersist();
  }

  /** Remove a node and all its connected edges. Triggers re-layout. */
  async removeNode(jobId: string): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.remove_node(this.canvasId, jobId);
    this.autoPersist();
  }

  /** Update a node's properties via partial JSON merge. */
  async updateNode(jobId: string, partial: Partial<Job>): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.update_node(this.canvasId, jobId, JSON.stringify(partial));
    this.autoPersist();
  }

  /** Add an edge between two nodes, optionally specifying ports. */
  async addEdge(fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.add_edge(this.canvasId, fromId, toId, fromPort ?? null, toPort ?? null, metadata ? JSON.stringify(metadata) : null);
    this.autoPersist();
  }

  /** Remove an edge between two nodes. */
  async removeEdge(fromId: string, toId: string): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.remove_edge(this.canvasId, fromId, toId);
    this.autoPersist();
  }

  /** Get all nodes in the graph. */
  async getNodes(): Promise<Job[]> {
    if (!this.alive) return [];
    const wasm = await ensureWasm();
    const result = wasm.get_nodes(this.canvasId);
    if (typeof result === 'string') {
      try { return JSON.parse(result) as Job[]; } catch { return []; }
    }
    return result ?? [];
  }

  /** Get all edges in the graph. */
  async getEdges(): Promise<EdgeInfo[]> {
    if (!this.alive) return [];
    const wasm = await ensureWasm();
    const result = wasm.get_edges(this.canvasId);
    if (typeof result === 'string') {
      try { return JSON.parse(result) as EdgeInfo[]; } catch { return []; }
    }
    return result ?? [];
  }

  // ─── Compound Node API ─────────────────────────────────────────────────────

  /** Group selected nodes into a compound node. Requires 2+ selected nodes. */
  async groupSelected(groupName: string = 'Group'): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.group_selected(this.canvasId, groupName);
  }

  /** Ungroup a compound node, restoring its children to the canvas. */
  async ungroupNode(nodeId: string): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.ungroup_node(this.canvasId, nodeId);
  }

  /** Toggle a compound node between collapsed and expanded. */
  async toggleCollapse(nodeId: string): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.toggle_collapse(this.canvasId, nodeId);
  }

  // ─── State Persistence API ──────────────────────────────────────────────────

  // ─── Node type registry ──────────────────────────────────────────────────

  /** Register a node type definition. The renderer uses this for colored headers, fields, etc. */
  registerNodeType(def: NodeDefinition): void {
    this.nodeTypeRegistry.set(def.type, def);
    this._syncDefToWasm(def);
  }

  /** Register multiple node type definitions at once. */
  registerNodeTypes(defs: NodeDefinition[]): void {
    for (const def of defs) {
      this.nodeTypeRegistry.set(def.type, def);
      this._syncDefToWasm(def);
    }
  }

  /** Send a NodeDefinition to WASM, mapping TS field names to Rust serde names. */
  private _syncDefToWasm(def: NodeDefinition): void {
    // Map TS interface names → Rust serde snake_case names
    const wasmDef = {
      node_type: def.type,
      label: def.label,
      icon: def.icon ?? '',
      header_color: def.headerColor ?? '',
      category: def.category ?? '',
      fields: (def.fields ?? []).map(f => ({
        key: f.key,
        field_type: f.type,
        label: f.label,
        options: f.options ?? [],
        default_value: f.defaultValue ?? null,
        min: f.min ?? null,
        max: f.max ?? null,
      })),
      inputs: def.inputs ?? [],
      outputs: def.outputs ?? [],
    };
    ensureWasm().then(wasm => {
      try {
        wasm.register_node_type(this.canvasId, JSON.stringify(wasmDef));
      } catch (e) {
        console.warn('Failed to register node type in WASM:', def.type, e);
      }
    });
  }

  /** Get the registered definition for a node type, or null if not registered. */
  getNodeType(type: string): NodeDefinition | null {
    return this.nodeTypeRegistry.get(type) ?? null;
  }

  /** Get all registered node type definitions. */
  getNodeTypes(): NodeDefinition[] {
    return Array.from(this.nodeTypeRegistry.values());
  }

  /** Convert canvas (graph-space) coordinates to screen (viewport) coordinates. */
  canvasToScreen(canvasX: number, canvasY: number): { x: number; y: number } {
    const rect = this.canvas.getBoundingClientRect();
    // TODO: incorporate zoom/pan transforms from WASM state
    // For now, return relative to viewport
    return {
      x: rect.left + canvasX,
      y: rect.top + canvasY,
    };
  }

  /** Get the full graph state for persistence (JSON-serializable). */
  async getState(): Promise<GraphState | null> {
    if (!this.alive) return null;
    const wasm = await ensureWasm();
    const result = wasm.get_state(this.canvasId);
    if (result === null || result === undefined) return null;
    // WASM returns a JSON string to avoid serde_wasm_bindgen Map issues
    if (typeof result === 'string') {
      try { return JSON.parse(result) as GraphState; } catch { return null; }
    }
    return result as GraphState;
  }

  /** Load a previously saved graph state. Restores nodes, positions, edges, zoom, pan. */
  async loadState(state: GraphState): Promise<void> {
    if (!this.alive) return;
    const wasm = await ensureWasm();
    wasm.load_state(this.canvasId, JSON.stringify(state));
  }

  // ─── DOM Node Overlay Layer ──────────────────────────────────────────────────

  /** Create the DOM overlay div that sits on top of the canvas. */
  private _createOverlayLayer(): void {
    this.nodeOverlayLayer = document.createElement('div');
    this.nodeOverlayLayer.style.cssText =
      'position:absolute;inset:0;pointer-events:none;overflow:hidden;z-index:1;';
    this.container.appendChild(this.nodeOverlayLayer);
    this._startOverlaySync();
  }

  /** rAF loop: sync DOM node positions with canvas zoom/pan/layout. */
  private _startOverlaySync(): void {
    const sync = () => {
      if (this.destroyed) return;
      this._syncOverlayPositions();
      this.nodeOverlayRafId = requestAnimationFrame(sync);
    };
    this.nodeOverlayRafId = requestAnimationFrame(sync);
  }

  /** Update DOM overlay positions from current WASM layout + viewport. */
  private _syncOverlayPositions(): void {
    if (!this.nodeOverlayLayer || !this.options.nodeRenderer || !this.alive) return;

    const wasm = this.wasmRef;
    if (!wasm) return;

    // Get current positions and nodes
    let positions: Record<string, [number, number]>;
    let nodes: Job[];
    let stateStr: string | GraphState | null;
    try {
      positions = wasm.get_node_positions(this.canvasId);
      nodes = wasm.get_nodes(this.canvasId);
      stateStr = wasm.get_state(this.canvasId);
    } catch {
      return; // Not initialized yet
    }

    let zoom = 1, panX = 0, panY = 0;
    if (stateStr) {
      const state: GraphState = typeof stateStr === 'string' ? JSON.parse(stateStr) : stateStr;
      zoom = state.zoom;
      panX = state.pan_x;
      panY = state.pan_y;
    }

    const nodeWidth = this.options.theme?.layout?.node_width ?? 200;
    const seen = new Set<string>();

    for (const node of nodes) {
      const pos = positions[node.id];
      if (!pos) continue;
      seen.add(node.id);

      let wrapper = this.nodeOverlayElements.get(node.id);
      if (!wrapper) {
        // Create new wrapper for this node
        wrapper = document.createElement('div');
        wrapper.style.cssText = 'position:absolute;pointer-events:auto;transform-origin:top left;';
        wrapper.dataset.nodeId = node.id;
        this.nodeOverlayLayer!.appendChild(wrapper);
        this.nodeOverlayElements.set(node.id, wrapper);
      }

      // Render content via consumer callback
      const def = this.getNodeType((node.metadata?.node_type as string) ?? '');
      const content = this.options.nodeRenderer!(node, def);

      if (!content) {
        // Consumer returned null — hide this overlay, let canvas render
        wrapper.style.display = 'none';
        continue;
      }

      wrapper.style.display = 'block';

      // Only replace DOM children if the content element changed
      if (wrapper.firstChild !== content) {
        wrapper.replaceChildren(content);
      }

      // Position: canvas coords → screen coords via zoom/pan
      const screenX = pos[0] * zoom + panX;
      const screenY = pos[1] * zoom + panY;

      wrapper.style.left = `${screenX}px`;
      wrapper.style.top = `${screenY}px`;
      wrapper.style.width = `${nodeWidth}px`;
      wrapper.style.transform = `scale(${zoom})`;
    }

    // Remove wrappers for nodes that no longer exist
    for (const [id, wrapper] of this.nodeOverlayElements) {
      if (!seen.has(id)) {
        wrapper.remove();
        this.nodeOverlayElements.delete(id);
      }
    }
  }

  /** Clean up event listeners, resize observer, and remove the canvas. */
  async destroy(): Promise<void> {
    this.destroyed = true;
    // Stop overlay sync loop
    cancelAnimationFrame(this.nodeOverlayRafId);
    // Remove overlay layer
    if (this.nodeOverlayLayer) {
      this.nodeOverlayLayer.remove();
      this.nodeOverlayLayer = null;
    }
    this.nodeOverlayElements.clear();
    // Disconnect JS-side ResizeObserver SYNCHRONOUSLY
    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
      this.resizeObserver = null;
    }
    // Mark WASM state as destroyed SYNCHRONOUSLY via cached ref.
    // This kills the animation loop and prevents any callbacks from
    // accessing stale canvas during the async gap below.
    if (this.wasmRef) {
      try { this.wasmRef.mark_destroyed(this.canvasId); } catch { /* ok */ }
    }
    // Async cleanup: remove WASM state and canvas from DOM
    try {
      const wasm = await ensureWasm();
      wasm.destroy(this.canvasId);
    } catch {
      // Ignore — WASM may already be in a bad state
    }
    this.canvas.remove();
    this.initialized = false;
  }

  /** Guard: check if this instance has been destroyed */
  private get alive(): boolean {
    return !this.destroyed;
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
