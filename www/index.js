import init, {
    render_workflow,
    update_workflow_data,
    select_node,
    deselect_all,
    reset_layout,
    zoom_to_fit,
} from './crates/web/pkg/github_graph_web.js';

const API_BASE = window.location.origin;
let currentWorkflowId = null;
let pollInterval = null;
let initialized = false;

// ─── Event callbacks ─────────────────────────────────────────────────────────

function onNodeClick(jobId) {
    console.log('Node clicked:', jobId);
    document.getElementById('status').textContent = `Clicked: ${jobId}`;
}

function onNodeHover(jobId) {
    // jobId is null when hover ends
    if (jobId) {
        console.log('Hovering:', jobId);
    }
}

function onCanvasClick() {
    console.log('Canvas clicked (empty space)');
}

function onSelectionChange(selectedIds) {
    console.log('Selection:', selectedIds);
}

function onNodeDragEnd(jobId, x, y) {
    console.log(`Node ${jobId} dragged to (${x.toFixed(0)}, ${y.toFixed(0)})`);
}

// ─── Render helpers ──────────────────────────────────────────────────────────

function renderGraph(json) {
    render_workflow(
        'graph', json,
        onNodeClick, onNodeHover, onCanvasClick,
        onSelectionChange, onNodeDragEnd,
    );
}

// ─── Initialization ──────────────────────────────────────────────────────────

async function initialize() {
    await init();

    const res = await fetch(`${API_BASE}/api/workflows`);
    const workflows = await res.json();

    if (workflows.length > 0) {
        currentWorkflowId = workflows[0].id;
        renderGraph(JSON.stringify(workflows[0]));
        initialized = true;
        updateStatus(workflows[0]);
        startPolling();
    }

    document.getElementById('run-btn').addEventListener('click', runWorkflow);
    document.getElementById('sample-btn').addEventListener('click', loadSample);
}

function updateStatus(workflow) {
    const statusEl = document.getElementById('status');
    const counts = {};
    for (const job of workflow.jobs) {
        counts[job.status] = (counts[job.status] || 0) + 1;
    }
    const parts = Object.entries(counts).map(([s, c]) => `${s}: ${c}`);
    statusEl.textContent = parts.join(' | ');
}

async function runWorkflow() {
    if (!currentWorkflowId) return;
    document.getElementById('status').textContent = 'Starting workflow...';
    await fetch(`${API_BASE}/api/workflows/${currentWorkflowId}/run`, { method: 'POST' });
}

async function loadSample() {
    document.getElementById('status').textContent = 'Loading sample...';
    const res = await fetch(`${API_BASE}/api/workflows/sample`, { method: 'POST' });
    if (res.ok) {
        const workflow = await res.json();
        currentWorkflowId = workflow.id;
        renderGraph(JSON.stringify(workflow));
        initialized = true;
        updateStatus(workflow);
    }
}

function startPolling() {
    if (pollInterval) clearInterval(pollInterval);
    pollInterval = setInterval(async () => {
        if (!currentWorkflowId) return;
        try {
            const res = await fetch(`${API_BASE}/api/workflows/${currentWorkflowId}/status`);
            if (res.ok) {
                const workflow = await res.json();
                const json = JSON.stringify(workflow);
                if (initialized) {
                    update_workflow_data('graph', json);
                } else {
                    renderGraph(json);
                    initialized = true;
                }
                updateStatus(workflow);
            }
        } catch (e) {
            // Server might be down, keep polling
        }
    }, 1000);
}

initialize().catch(console.error);
