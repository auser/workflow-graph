/**
 * @workflow-graph/client — TypeScript client for the workflow-graph REST API
 */

export interface Workflow {
  id: string;
  name: string;
  trigger: string;
  jobs: Job[];
}

export interface Job {
  id: string;
  name: string;
  status: string;
  command: string;
  duration_secs?: number;
  started_at?: number;
  depends_on: string[];
  output?: string;
}

export interface LogChunk {
  workflow_id: string;
  job_id: string;
  sequence: number;
  data: string;
  timestamp_ms: number;
  stream: 'stdout' | 'stderr';
}

export interface WorkerInfo {
  worker_id: string;
  labels: string[];
  registered_at_ms: number;
  last_heartbeat_ms: number;
  current_job: string | null;
  status: 'idle' | 'busy' | 'offline';
}

/** Error thrown when the API returns a non-OK response. */
export class WorkflowApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly statusText: string,
  ) {
    super(message);
    this.name = 'WorkflowApiError';
  }
}

/** Assert response is OK, throw WorkflowApiError otherwise. */
async function assertOk(res: Response, context: string): Promise<void> {
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new WorkflowApiError(
      `${context}: ${res.status} ${res.statusText}${body ? ` — ${body}` : ''}`,
      res.status,
      res.statusText,
    );
  }
}

/**
 * Client for the workflow-graph REST API.
 *
 * @example
 * ```typescript
 * const client = new WorkflowClient('http://localhost:3000');
 * const workflows = await client.listWorkflows();
 * await client.runWorkflow(workflows[0].id);
 * ```
 */
export class WorkflowClient {
  constructor(private baseUrl: string) {}

  async listWorkflows(): Promise<Workflow[]> {
    const res = await fetch(`${this.baseUrl}/api/workflows`);
    await assertOk(res, 'listWorkflows');
    return res.json() as Promise<Workflow[]>;
  }

  async getStatus(id: string): Promise<Workflow> {
    const res = await fetch(`${this.baseUrl}/api/workflows/${encodeURIComponent(id)}/status`);
    await assertOk(res, `getStatus(${id})`);
    return res.json() as Promise<Workflow>;
  }

  async createWorkflow(workflow: Omit<Workflow, 'id'>): Promise<Workflow> {
    const res = await fetch(`${this.baseUrl}/api/workflows`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(workflow),
    });
    await assertOk(res, 'createWorkflow');
    return res.json() as Promise<Workflow>;
  }

  async runWorkflow(id: string): Promise<void> {
    const res = await fetch(`${this.baseUrl}/api/workflows/${encodeURIComponent(id)}/run`, {
      method: 'POST',
    });
    await assertOk(res, `runWorkflow(${id})`);
  }

  async cancelWorkflow(id: string): Promise<void> {
    const res = await fetch(`${this.baseUrl}/api/workflows/${encodeURIComponent(id)}/cancel`, {
      method: 'POST',
    });
    await assertOk(res, `cancelWorkflow(${id})`);
  }

  async getJobLogs(workflowId: string, jobId: string): Promise<LogChunk[]> {
    const wfId = encodeURIComponent(workflowId);
    const jId = encodeURIComponent(jobId);
    const res = await fetch(`${this.baseUrl}/api/workflows/${wfId}/jobs/${jId}/logs`);
    await assertOk(res, `getJobLogs(${workflowId}, ${jobId})`);
    return res.json() as Promise<LogChunk[]>;
  }

  /**
   * Stream job logs via SSE. Returns an async iterable of log chunks.
   */
  async *streamLogs(workflowId: string, jobId: string): AsyncIterable<LogChunk> {
    const wfId = encodeURIComponent(workflowId);
    const jId = encodeURIComponent(jobId);
    const url = `${this.baseUrl}/api/workflows/${wfId}/jobs/${jId}/logs/stream`;
    const eventSource = new EventSource(url);

    const chunks: LogChunk[] = [];
    let resolve: (() => void) | null = null;
    let done = false;

    eventSource.addEventListener('log', (event: MessageEvent) => {
      chunks.push(JSON.parse(event.data) as LogChunk);
      if (resolve) {
        resolve();
        resolve = null;
      }
    });

    eventSource.addEventListener('error', () => {
      done = true;
      eventSource.close();
      if (resolve) {
        resolve();
        resolve = null;
      }
    });

    try {
      while (!done) {
        while (chunks.length > 0) {
          yield chunks.shift()!;
        }
        if (!done) {
          await new Promise<void>((r) => {
            resolve = r;
          });
        }
      }
    } finally {
      eventSource.close();
    }
  }

  async listWorkers(): Promise<WorkerInfo[]> {
    const res = await fetch(`${this.baseUrl}/api/workers`);
    await assertOk(res, 'listWorkers');
    return res.json() as Promise<WorkerInfo[]>;
  }
}
