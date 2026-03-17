# Vanilla JS Example

Demonstrates `@auser/workflow-graph-web` with plain HTML + ES modules via Vite.

## Features shown

- Dark / light / high-contrast theme switching
- Minimap toggle
- Layout direction toggle (left-to-right / top-to-bottom)
- Edge click and node selection callbacks
- Auto-resize
- Static sample data (no server needed)

## Run

```bash
cd examples/vanilla-web
npm install
npm run dev
```

Open `http://localhost:5173` in your browser.

## What you'll see

An 8-node CI pipeline graph with:
- 3 completed jobs (green checks)
- 1 running job (animated spinner)
- 4 queued jobs (gray circles)

Use the buttons to switch themes, toggle the minimap, and change layout direction.
