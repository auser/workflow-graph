---
title: WASM API
description: JavaScript API for the WebAssembly workflow graph renderer
---

The WASM module provides an interactive canvas-based workflow graph renderer. It's available directly or via the `@workflow-graph/web` NPM package.

## Setup

```javascript
import init, {
  render_workflow,
  update_workflow_data,
  select_node,
  deselect_all,
  reset_layout,
  zoom_to_fit,
  set_zoom,
  get_node_positions,
  set_node_positions,
  destroy,
} from 'workflow-graph-web';

await init();
```

Or via the NPM wrapper:

```javascript
import { WorkflowGraph } from '@workflow-graph/web';

const graph = new WorkflowGraph('canvas-id');
await graph.init();
```

## Rendering

### `render_workflow(canvas_id, workflow_json, ...callbacks)`

Renders a workflow graph onto a canvas element.

```javascript
render_workflow(
  'canvas-id',
  workflowJson,
  (jobId) => console.log('clicked', jobId),       // on_node_click
  (jobId) => console.log('hover', jobId),          // on_node_hover
  () => console.log('canvas clicked'),             // on_canvas_click
  (ids) => console.log('selected', ids),           // on_selection_change
  (id, x, y) => console.log('dragged', id, x, y), // on_node_drag_end
);
```

### `update_workflow_data(canvas_id, workflow_json)`

Updates the workflow data without resetting positions, zoom, or selection. Use this for polling status updates.

```javascript
update_workflow_data('canvas-id', newWorkflowJson);
```

## Selection

### `select_node(canvas_id, job_id)`

Programmatically select a node.

```javascript
select_node('canvas-id', 'build');
```

### `deselect_all(canvas_id)`

Clear all selections.

```javascript
deselect_all('canvas-id');
```

## Viewport Control

### `reset_layout(canvas_id)`

Reset node positions to the default DAG layout.

### `zoom_to_fit(canvas_id)`

Adjust zoom and pan to fit all nodes in view.

### `set_zoom(canvas_id, level)`

Set zoom level (0.25 to 4.0).

```javascript
set_zoom('canvas-id', 1.5);
```

## Position Persistence

### `get_node_positions(canvas_id) → JSON`

Returns a JSON string of all node positions. Use this to persist layout across sessions.

### `set_node_positions(canvas_id, positions_json)`

Restore previously saved node positions.

```javascript
const positions = get_node_positions('canvas-id');
localStorage.setItem('positions', positions);

// Later:
set_node_positions('canvas-id', localStorage.getItem('positions'));
```

## Cleanup

### `destroy(canvas_id)`

Remove all event listeners and free resources for a canvas.

## Visualization Features

- **GitHub-accurate icons** — Octicon SVG icons rendered via Canvas Path2D
- **Status indicators** — queued (hollow circle), running (animated spinning ring), success (green check), failure (red X), skipped (gray slash), cancelled
- **Animated timers** — running jobs show a live elapsed timer
- **Path highlighting** — hover a node to see its upstream/downstream dependencies highlighted in blue
- **Pan & zoom** — mouse wheel to zoom (0.25x–4x), click+drag empty space to pan
- **Multi-select** — shift+click to select multiple nodes
- **Drag & drop** — reposition nodes by dragging
