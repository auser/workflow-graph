# React Example

Demonstrates `@auser/workflow-graph-react` with a full-featured React app.

## Features shown

- `WorkflowGraphComponent` with `ref` for imperative control
- Theme switching (dark / light / high-contrast)
- Minimap toggle
- Custom per-edge styles (dashed orange deploy edge)
- Node click with state tracking
- Error handling via `onError` prop
- Auto-resize
- Loading skeleton (automatic)

## Run

```bash
npm create vite@latest my-app -- --template react-ts
cd my-app
npm install @auser/workflow-graph-react @auser/workflow-graph-web
# Copy App.tsx into src/App.tsx
npm run dev
```
