# React Example

Demonstrates `@auser/workflow-graph-react` with a Vite + React app.

## Features shown

- `WorkflowGraphComponent` with `ref` for imperative control
- Theme switching (dark / light / high-contrast)
- Minimap toggle
- Custom per-edge styles (dashed orange deploy edge)
- Node click with state tracking
- Error handling via `onError` prop
- Auto-resize
- Loading skeleton (automatic)
- Static sample data (no server needed)

## Run

```bash
cd examples/react-app
npm install
npm run dev
```

Open `http://localhost:5173` in your browser.

## What you'll see

A 6-node CI pipeline graph with theme switching buttons, minimap toggle, and zoom controls. Click nodes to see selection state. The `build → deploy` edge is styled with a dashed orange line.
