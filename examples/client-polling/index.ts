/**
 * Client polling example — @auser/workflow-graph-client
 *
 * Demonstrates:
 * - Creating and running a workflow via the REST API
 * - Polling for status updates
 * - Streaming job logs via SSE
 * - Error handling with WorkflowApiError
 */

import { WorkflowClient, WorkflowApiError } from '@auser/workflow-graph-client';
export {}; // ensure this is treated as a module

const PORT = process.env.PORT ?? `3000`;
const SERVER = process.env.SERVER_URL ?? `http://localhost:${PORT}`;
const client = new WorkflowClient(SERVER);

async function main() {
  console.log('Connecting to', SERVER);

  // List existing workflows
  const workflows = await client.listWorkflows();
  console.log(`Found ${workflows.length} workflow(s)`);

  if (workflows.length === 0) {
    console.log('No workflows found. Start the server with `just dev` first.');
    return;
  }

  const workflow = workflows[0];
  console.log(`Using workflow: ${workflow.name} (${workflow.id})`);

  // Run the workflow
  try {
    await client.runWorkflow(workflow.id);
    console.log('Workflow started!');
  } catch (err) {
    if (err instanceof WorkflowApiError && err.status === 404) {
      console.error('Workflow not found:', workflow.id);
      return;
    }
    throw err;
  }

  // Poll for status updates
  console.log('\nPolling for status...\n');
  let allDone = false;

  while (!allDone) {
    const status = await client.getStatus(workflow.id);

    const summary = status.jobs.map((j) => {
      const icon =
        j.status === 'success' ? '✓' :
        j.status === 'failure' ? '✗' :
        j.status === 'running' ? '⟳' :
        j.status === 'skipped' ? '−' :
        '○';
      const duration = j.duration_secs ? ` (${j.duration_secs}s)` : '';
      return `  ${icon} ${j.name}: ${j.status}${duration}`;
    });

    // Clear and reprint
    console.clear();
    console.log(`Workflow: ${status.name}\n`);
    console.log(summary.join('\n'));

    allDone = status.jobs.every((j) =>
      ['success', 'failure', 'skipped', 'cancelled'].includes(j.status)
    );

    if (!allDone) {
      await new Promise((r) => setTimeout(r, 1000));
    }
  }

  console.log('\nWorkflow complete!\n');

  // Show logs for a completed job
  const completedJob = (await client.getStatus(workflow.id)).jobs.find(
    (j) => j.status === 'success' || j.status === 'failure'
  );

  if (completedJob) {
    console.log(`Logs for ${completedJob.name}:`);
    const logs = await client.getJobLogs(workflow.id, completedJob.id);
    for (const chunk of logs) {
      process.stdout.write(chunk.data);
    }
  }
}

main().catch((err) => {
  if (err instanceof WorkflowApiError) {
    console.error(`API Error ${err.status}: ${err.message}`);
  } else {
    console.error(err);
  }
  process.exit(1);
});
