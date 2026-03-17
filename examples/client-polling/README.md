# Client Polling Example

Demonstrates `@auser/workflow-graph-client` for server-side workflow management.

## Features shown

- List workflows from the REST API
- Run a workflow
- Poll for status updates with live terminal output
- Fetch job logs after completion
- Error handling with `WorkflowApiError`

## Run

Start the server first (on a free port):

```bash
# In the workflow-graph repo root
PORT=4000 just dev
```

Then in this directory:

```bash
cd examples/client-polling
npm install
SERVER_URL=http://localhost:4000 npm start
```

You'll see a live-updating status display as jobs transition from queued → running → success.
