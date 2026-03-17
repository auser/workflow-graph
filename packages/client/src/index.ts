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
    return res.json();
  }

  async getStatus(id: string): Promise<Workflow> {
    const res = await fetch(`${this.baseUrl}/api/workflows/${id}/status`);
    if (!res.ok) throw new Error(`Workflow ${id} not found`);
    return res.json();
  }

  async createWorkflow(workflow: Omit<Workflow, 'id'>): Promise<Workflow> {
    const res = await fetch(`${this.baseUrl}/api/workflows`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(workflow),
    });
    return res.json();
  }

  async runWorkflow(id: string): Promise<void> {
    await fetch(`${this.baseUrl}/api/workflows/${id}/run`, { method: 'POST' });
  }

  async cancelWorkflow(id: string): Promise<void> {
    await fetch(`${this.baseUrl}/api/workflows/${id}/cancel`, { method: 'POST' });
  }

  async getJobLogs(workflowId: string, jobId: string): Promise<LogChunk[]> {
    const res = await fetch(`${this.baseUrl}/api/workflows/${workflowId}/jobs/${jobId}/logs`);
    return res.json();
  }

  /**
   * Stream job logs via SSE. Returns an async iterable of log chunks.
   */
  async *streamLogs(workflowId: string, jobId: string): AsyncIterable<LogChunk> {
    const url = `${this.baseUrl}/api/workflows/${workflowId}/jobs/${jobId}/logs/stream`;
    const eventSource = new EventSource(url);

    const chunks: LogChunk[] = [];
    let resolve: (() => void) | null = null;
    let done = false;

    eventSource.addEventListener('log', (event: MessageEvent) => {
      chunks.push(JSON.parse(event.data));
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
          await new Promise<void>((r) => { resolve = r; });
        }
      }
    } finally {
      eventSource.close();
    }
  }

  async listWorkers(): Promise<WorkerInfo[]> {
    const res = await fetch(`${this.baseUrl}/api/workers`);
    return res.json();
  }
}
