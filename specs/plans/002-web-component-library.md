# Web Component Library — Make github-graph a drop-in UI library

## Context
The WASM crate currently works but isn't usable as a library by external consumers. It has hardcoded theming, limited events, no zoom/pan, no selection state, no npm packaging, and no framework adapters. This plan covers everything needed to make it a production-grade web component.

## Deliverables

### 1. Configuration API
- Accept a `GraphConfig` object in `render_workflow()`:
  - Theme: node colors, font, sizing, border radius, edge colors
  - Layout: direction (LR/TB), gap sizes, padding
  - Behavior: drag enabled, zoom enabled, animation speed
- Pass as JSON string from JS, deserialize in Rust
- Defaults for everything (zero-config works out of the box)

### 2. Event Callbacks
Extend `render_workflow()` options:
- `on_node_click(job_id: string)`
- `on_node_hover(job_id: string | null)` — null when hover ends
- `on_node_drag_end(job_id: string, x: number, y: number)`
- `on_edge_click(from_id: string, to_id: string)`
- `on_canvas_click()` — click on empty space
- `on_selection_change(selected_ids: string[])`

### 3. Programmatic Control API
Exported WASM functions:
- `select_node(canvas_id, job_id)` — highlight with selection ring
- `deselect_all(canvas_id)`
- `reset_layout(canvas_id)` — snap to auto-computed positions
- `zoom_to_fit(canvas_id)`
- `set_zoom(canvas_id, level)` — 0.1 to 3.0
- `get_node_positions(canvas_id) -> JSON` — export for persistence
- `set_node_positions(canvas_id, positions_json)` — restore
- `destroy(canvas_id)` — remove event listeners, clean up state

### 4. Pan & Zoom
- Mouse wheel: zoom in/out centered on cursor
- Click+drag on empty canvas space: pan
- Pinch-to-zoom on touch devices
- Zoom level clamped (0.25x to 4x)
- Transform matrix stored in GraphState

### 5. Selection State
- Click node → selected (highlighted border, slightly raised shadow)
- Shift+click → toggle add to selection
- Click empty space → deselect all
- Selected nodes visually distinct (blue border ring, like GitHub's focus state)
- `on_selection_change` callback fires with array of selected IDs

### 6. Accessibility
- Canvas has `role="img"` and `aria-label`
- Hidden DOM overlay with focusable elements per node
- Tab navigation between nodes
- Enter/Space to select
- Arrow keys to navigate the DAG structure

### 7. NPM Package
- Publish as `@github-graph/web` (or `github-graph`)
- TypeScript type definitions generated from WASM bindings
- Wrapper class:
  ```typescript
  class WorkflowGraph {
    constructor(element: HTMLElement, options?: GraphOptions)
    setWorkflow(data: Workflow): void
    updateStatus(data: Workflow): void
    selectNode(jobId: string): void
    resetLayout(): void
    zoomToFit(): void
    destroy(): void
    on(event: string, callback: Function): void
  }
  ```
- Auto-init WASM, creates canvas element inside container
- Framework adapters:
  - React: `<WorkflowGraph workflow={data} onNodeClick={fn} />`
  - Vanilla JS: `new WorkflowGraph(document.getElementById('container'))`

### 8. Client SDK
- `@github-graph/client` npm package
- TypeScript client for the REST API:
  ```typescript
  class WorkflowClient {
    constructor(baseUrl: string)
    listWorkflows(): Promise<Workflow[]>
    getStatus(id: string): Promise<Workflow>
    runWorkflow(id: string): Promise<void>
    cancelWorkflow(id: string): Promise<void>
    streamLogs(wfId: string, jobId: string): AsyncIterable<LogChunk>
  }
  ```

### 9. Error Handling
- `on_error(error: string)` callback
- Graceful WASM load failure (show fallback message)
- Workflow JSON validation with clear error messages

### 10. Documentation
- README with quick start for vanilla JS, React, Vue
- API reference (auto-generated from TypeScript types)
- Architecture guide for custom queue backends

## Implementation Order
1. Config API + all event callbacks
2. Pan & zoom
3. Selection state
4. Programmatic control API
5. NPM wrapper class + TypeScript types
6. React adapter
7. Client SDK
8. Accessibility
9. Documentation
