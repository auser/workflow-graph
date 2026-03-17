---
title: WASM API
description: JavaScript API for the WebAssembly workflow graph renderer
---

The WASM module provides an interactive canvas-based workflow graph renderer. It's available directly or via the `@auser/workflow-graph-web` NPM package.

## Setup

### NPM Package (recommended)

```typescript
import { WorkflowGraph, darkTheme } from '@auser/workflow-graph-web';

const graph = new WorkflowGraph(document.getElementById('container')!, {
  onNodeClick: (jobId) => console.log('clicked', jobId),
  theme: darkTheme,
  autoResize: true,
});
await graph.setWorkflow(workflowData);
```

### React Component

```tsx
import { WorkflowGraphComponent, darkTheme } from '@auser/workflow-graph-react';
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

## Cleanup

### `destroy(canvas_id)`

Remove all event listeners, ARIA live region, resize observer, and free resources. Call this when unmounting the component.

## Interaction Features

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
