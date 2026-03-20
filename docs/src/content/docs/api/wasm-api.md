---
title: WASM API
description: JavaScript API for the WebAssembly workflow graph renderer
---

The WASM module provides an interactive canvas-based workflow graph renderer. It's available directly or via the `@auser/workflow-graph-web` NPM package.

:::tip[Examples]
See the [examples directory](https://github.com/auser/workflow-graph/tree/main/examples) for complete working apps using each package.
:::

## Setup

### NPM Package (recommended)

```typescript
import { WorkflowGraph, darkTheme, setWasmUrl } from '@auser/workflow-graph-web';

// Required for Vite; recommended for all bundlers
setWasmUrl('/workflow_graph_web_bg.wasm');

const graph = new WorkflowGraph(document.getElementById('container')!, {
  onNodeClick: (jobId) => console.log('clicked', jobId),
  theme: darkTheme,
  autoResize: true,
});
await graph.setWorkflow(workflowData);
```

:::note[WASM Binary]
Copy `workflow_graph_web_bg.wasm` from `node_modules/@auser/workflow-graph-web/wasm/` to your `public/` directory, or configure your bundler to serve it. Then call `setWasmUrl()` with the URL before creating any graph instances.
:::

### React Component

```tsx
import { setWasmUrl } from '@auser/workflow-graph-web';
import { WorkflowGraphComponent, darkTheme } from '@auser/workflow-graph-react';

setWasmUrl('/workflow_graph_web_bg.wasm');
import type { WorkflowGraphHandle } from '@auser/workflow-graph-react';

const ref = useRef<WorkflowGraphHandle>(null);

<WorkflowGraphComponent
  ref={ref}
  workflow={workflowData}
  theme={darkTheme}
  autoResize
  onNodeClick={(id) => console.log(id)}
  onError={(err) => console.error(err)}
/>

// Imperative control
ref.current?.zoomToFit();
ref.current?.setTheme(lightTheme);
```

### Raw WASM

```javascript
import init, {
  render_workflow,
  update_workflow_data,
  set_theme,
  set_auto_resize,
  set_on_edge_click,
  set_on_render_node,
  select_node,
  deselect_all,
  reset_layout,
  zoom_to_fit,
  set_zoom,
  get_node_positions,
  set_node_positions,
  add_node,
  remove_node,
  update_node,
  add_edge,
  remove_edge,
  get_nodes,
  get_edges,
  destroy,
} from 'workflow-graph-web';

await init();
```

## Rendering

### `render_workflow(canvas_id, workflow_json, ...callbacks, theme_json?)`

Renders a workflow graph onto a canvas element. The optional `theme_json` parameter accepts a JSON-serialized `ThemeConfig`.

```javascript
render_workflow(
  'canvas-id',
  workflowJson,
  (jobId) => console.log('clicked', jobId),       // on_node_click
  (jobId) => console.log('hover', jobId),          // on_node_hover
  () => console.log('canvas clicked'),             // on_canvas_click
  (ids) => console.log('selected', ids),           // on_selection_change
  (id, x, y) => console.log('dragged', id, x, y), // on_node_drag_end
  themeJson,                                        // theme (optional)
);
```

### `update_workflow_data(canvas_id, workflow_json)`

Updates the workflow data without resetting positions, zoom, or selection. Use this for polling status updates. Announces status changes to screen readers via ARIA live region.

## Theming

### `set_theme(canvas_id, theme_json)`

Update the theme at runtime. Triggers automatic re-layout if dimensions or direction changed.

```javascript
// Switch to dark mode
set_theme('canvas-id', JSON.stringify({
  colors: {
    bg: '#0d1117',
    node_bg: '#161b22',
    text: '#e6edf3',
    // ... see ThemeConfig for all options
  }
}));
```

### Theme Presets

Three built-in presets are available in TypeScript:

```typescript
import { darkTheme, lightTheme, highContrastTheme } from '@auser/workflow-graph-web';

// darkTheme — GitHub dark mode colors
// lightTheme — default (no-op)
// highContrastTheme — WCAG AA 4.5:1+ contrast
```

### ThemeConfig

All fields are optional — omitted fields use light-theme defaults.

| Field | Type | Description |
|-------|------|-------------|
| `colors` | `ThemeColors` | Status, node, graph, header colors |
| `fonts` | `ThemeFonts` | Font family and sizes |
| `layout` | `ThemeLayout` | Node dimensions, gaps, padding |
| `direction` | `"LeftToRight" \| "TopToBottom"` | DAG flow direction |
| `labels` | `Labels` | Localized status labels and duration formats |
| `edge_styles` | `Record<string, EdgeStyle>` | Per-edge style overrides (keyed by `"from->to"`) |
| `minimap` | `boolean` | Show minimap overlay (default: false) |

### Labels (i18n)

Override status text and duration formats for localization:

```typescript
const theme: ThemeConfig = {
  labels: {
    queued: 'En cola',
    running: 'Ejecutando',
    success: 'Exitoso',
    failure: 'Error',
    skipped: 'Omitido',
    cancelled: 'Cancelado',
    duration_minutes: '{m}min {s}s',
    duration_seconds: '{s}s',
  },
};
```

### Edge Styles

Customize individual edges by `"fromId->toId"` key:

```typescript
const theme: ThemeConfig = {
  edge_styles: {
    'build->deploy': { color: '#ff0000', width: 3, dash: [5, 3] },
    'test->build': { color: '#00ff00' },
  },
};
```

## Minimap

Enable a 160x100px overview in the bottom-right corner:

```typescript
const theme: ThemeConfig = { minimap: true };
```

The minimap shows status-colored nodes and a viewport indicator rectangle.

## Custom Node Rendering

### `set_on_render_node(canvas_id, callback)`

Replace or augment default node rendering with a custom callback:

```typescript
set_on_render_node('canvas-id', (x, y, w, h, job) => {
  // Draw custom content on the canvas...
  // Return true to skip default rendering, false to draw default on top
  return false;
});
```

## Edge Click

### `set_on_edge_click(canvas_id, callback)`

Detect clicks on edges (bezier curve hit testing):

```typescript
set_on_edge_click('canvas-id', (fromId, toId) => {
  console.log(`Edge clicked: ${fromId} -> ${toId}`);
});
```

## Auto Resize

### `set_auto_resize(canvas_id, enabled)`

Automatically resize the canvas when its parent container changes size (via `ResizeObserver`):

```javascript
set_auto_resize('canvas-id', true);
```

## Selection

### `select_node(canvas_id, job_id)`

Programmatically select a node.

### `deselect_all(canvas_id)`

Clear all selections.

## Viewport Control

### `reset_layout(canvas_id)`

Reset node positions to the default DAG layout.

### `zoom_to_fit(canvas_id)`

Adjust zoom and pan to fit all nodes in view.

### `set_zoom(canvas_id, level)`

Set zoom level (0.25 to 4.0).

## Position Persistence

### `get_node_positions(canvas_id) → JSON`

Returns a JSON object of all node positions. Use this to persist layout across sessions.

### `set_node_positions(canvas_id, positions_json)`

Restore previously saved node positions.

## Node CRUD API

Dynamically add, remove, and update nodes and edges at runtime. All mutations trigger automatic re-layout and re-render.

### `add_node(canvas_id, job_json)`

Add a new node to the graph. The `job_json` string must be a valid JSON-serialized `Job` object. Throws if a node with the same ID already exists.

```typescript
// Raw WASM
add_node('canvas-id', JSON.stringify({
  id: 'new-job',
  name: 'New Job',
  status: 'queued',
  command: 'echo hello',
  depends_on: ['build'],
  metadata: { icon: 'rocket', priority: 1 },
}));

// TypeScript wrapper
await graph.addNode({
  id: 'new-job',
  name: 'New Job',
  status: 'queued',
  command: 'echo hello',
  depends_on: ['build'],
  metadata: { icon: 'rocket', priority: 1 },
});
```

### `remove_node(canvas_id, job_id)`

Remove a node and all its connected edges. Throws if the node doesn't exist.

```typescript
// Raw WASM
remove_node('canvas-id', 'new-job');

// TypeScript wrapper
await graph.removeNode('new-job');
```

### `update_node(canvas_id, job_id, partial_json)`

Update a node's properties via JSON merge. Only the provided fields are changed — omitted fields keep their current values. Supports updating `name`, `status`, `command`, and `metadata` (metadata is merged, not replaced).

```typescript
// Raw WASM
update_node('canvas-id', 'new-job', JSON.stringify({
  status: 'running',
  metadata: { started_by: 'user-123' },
}));

// TypeScript wrapper
await graph.updateNode('new-job', {
  status: 'running',
  metadata: { started_by: 'user-123' },
});
```

### `add_edge(canvas_id, from_id, to_id, metadata_json?)`

Add an edge between two existing nodes. The edge represents a dependency: `to_id` depends on `from_id`. Duplicate edges are silently ignored. Throws if either node doesn't exist.

```typescript
// Raw WASM
add_edge('canvas-id', 'build', 'deploy', JSON.stringify({ label: 'on success' }));

// TypeScript wrapper
await graph.addEdge('build', 'deploy', { label: 'on success' });
```

### `remove_edge(canvas_id, from_id, to_id)`

Remove an edge between two nodes.

```typescript
// Raw WASM
remove_edge('canvas-id', 'build', 'deploy');

// TypeScript wrapper
await graph.removeEdge('build', 'deploy');
```

### `get_nodes(canvas_id) → Job[]`

Returns all nodes in the graph as an array of `Job` objects.

```typescript
// Raw WASM
const nodes = get_nodes('canvas-id');

// TypeScript wrapper
const nodes = await graph.getNodes();
```

### `get_edges(canvas_id) → EdgeInfo[]`

Returns all edges as an array of `{ from_id, to_id, metadata }` objects.

```typescript
// Raw WASM
const edges = get_edges('canvas-id');

// TypeScript wrapper
const edges = await graph.getEdges();
```

### React Ref API

All Node CRUD methods are available on the `WorkflowGraphHandle` ref:

```tsx
const ref = useRef<WorkflowGraphHandle>(null);

await ref.current?.addNode({ id: 'deploy', name: 'Deploy', status: 'queued', command: './deploy.sh', depends_on: ['build'] });
await ref.current?.addEdge('build', 'deploy');
await ref.current?.updateNode('deploy', { status: 'running' });
const nodes = await ref.current?.getNodes();
const edges = await ref.current?.getEdges();
await ref.current?.removeEdge('build', 'deploy');
await ref.current?.removeNode('deploy');
```

## Types

### `Job`

```typescript
interface Job {
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
}
```

### `EdgeInfo`

```typescript
interface EdgeInfo {
  from_id: string;
  to_id: string;
  metadata?: Record<string, unknown>;
}
```

## Cleanup

### `destroy(canvas_id)`

Remove all event listeners, ARIA live region, resize observer, and free resources. Call this when unmounting the component.

## Interaction Features

- **Node CRUD** — add, remove, update nodes and edges at runtime with automatic re-layout
- **Node & edge metadata** — attach arbitrary key-value data to nodes and edges for custom renderers
- **GitHub-accurate icons** — Octicon SVG icons rendered via Canvas Path2D
- **Status indicators** — queued (hollow circle), running (animated spinning ring), success (green check), failure (red X), skipped (gray slash), cancelled
- **Animated timers** — running jobs show a live elapsed timer
- **Path highlighting** — hover a node to see its upstream/downstream dependencies highlighted in blue
- **Pan & zoom** — mouse wheel to zoom (0.25x–4x), click+drag empty space to pan
- **Touch support** — touch drag, pan, and tap on mobile devices
- **Multi-select** — shift+click to select multiple nodes
- **Drag & drop** — reposition nodes by dragging
- **Keyboard navigation** — Tab/Shift+Tab to cycle nodes, Enter/Space to activate, Escape to deselect
- **Accessibility** — ARIA live region announces status changes to screen readers
- **HiDPI** — automatically adapts to `devicePixelRatio` changes (multi-monitor)
