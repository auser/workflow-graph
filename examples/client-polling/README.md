# Client Polling Example

Demonstrates `@auser/workflow-graph-client` for server-side workflow management.

## Features shown

- List workflows from the REST API
- Run a workflow
- Poll for status updates with live terminal output
- Fetch job logs after completion
- Error handling with `WorkflowApiError`

## Run

Start the server first:

```bash
# In the workflow-graph repo root
just dev
```

Then in another terminal:

```bash
npm install @auser/workflow-graph-client
npx tsx index.ts
```

You'll see a live-updating status display as jobs transition from queued → running → success.
