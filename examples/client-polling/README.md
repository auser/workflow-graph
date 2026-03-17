# Client Polling Example

Demonstrates `@auser/workflow-graph-client` for server-side workflow management.

## Features shown

- List workflows from the REST API
- Run a workflow
- Poll for status updates with live terminal output
- Fetch job logs after completion
- Error handling with `WorkflowApiError`

## Run

You need three terminals:

**Terminal 1 — Server:**

```bash
# In the workflow-graph repo root
PORT=4000 just dev
```

**Terminal 2 — Worker:**

```bash
# In the workflow-graph repo root
SERVER_URL=http://localhost:4000 cargo run -p workflow-graph-worker-sdk
```

**Terminal 3 — Client:**

```bash
cd examples/client-polling
npm install
npm start
```

You'll see a live-updating status display as jobs transition from queued → running → success.

## Configuration

Override the server URL:

```bash
SERVER_URL=http://localhost:5000 npm start
```

Or set the port (defaults to 4000):

```bash
PORT=5000 npm start
```
