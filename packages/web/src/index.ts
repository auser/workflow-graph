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
}

export interface GraphOptions {
  onNodeClick?: (jobId: string) => void;
  onNodeHover?: (jobId: string | null) => void;
  onCanvasClick?: () => void;
  onSelectionChange?: (selectedIds: string[]) => void;
  onNodeDragEnd?: (jobId: string, x: number, y: number) => void;
}

let wasmModule: any = null;

async function ensureWasm(): Promise<any> {
  if (wasmModule) return wasmModule;
  // Dynamic import of the WASM package
  wasmModule = await import('../../crates/web/pkg/workflow_graph_web.js');
  await wasmModule.default();
  return wasmModule;
}

/**
 * Interactive workflow DAG graph component.
 *
 * @example
 * ```typescript
 * const graph = new WorkflowGraph(document.getElementById('container')!, {
 *   onNodeClick: (jobId) => console.log('clicked', jobId),
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
    wasm.render_workflow(
      this.canvasId,
      json,
      this.options.onNodeClick || null,
      this.options.onNodeHover || null,
      this.options.onCanvasClick || null,
      this.options.onSelectionChange || null,
      this.options.onNodeDragEnd || null,
    );
    this.initialized = true;
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

  /** Clean up event listeners and remove the canvas. */
  async destroy(): Promise<void> {
    const wasm = await ensureWasm();
    wasm.destroy(this.canvasId);
    this.canvas.remove();
    this.initialized = false;
  }
}
