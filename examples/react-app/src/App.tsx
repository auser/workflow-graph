/**
 * React example — @auser/workflow-graph-react
 *
 * Demonstrates:
 * - WorkflowGraphComponent with ref for imperative control
 * - Theme switching (dark / light / high-contrast)
 * - Loading skeleton
 * - Error handling
 * - Auto-resize
 * - Custom edge styles
 */

import { useRef, useState, useCallback } from 'react';
import { setWasmUrl } from '@auser/workflow-graph-web';

// Tell the WASM loader where to find the binary (served from publicDir)
setWasmUrl('/workflow_graph_web_bg.wasm');

import {
  WorkflowGraphComponent,
  darkTheme,
  lightTheme,
  highContrastTheme,
} from '@auser/workflow-graph-react';
import type {
  WorkflowGraphHandle,
  Workflow,
  ThemeConfig,
} from '@auser/workflow-graph-react';

const sampleWorkflow: Workflow = {
  id: 'ci-1',
  name: 'CI Pipeline',
  trigger: 'on: push',
  jobs: [
    { id: 'lint', name: 'Lint', status: 'success', command: '', depends_on: [], duration_secs: 12 },
    { id: 'test', name: 'Unit Tests', status: 'success', command: '', depends_on: [], duration_secs: 45 },
    { id: 'typecheck', name: 'Typecheck', status: 'success', command: '', depends_on: [], duration_secs: 8 },
    { id: 'build', name: 'Build', status: 'running', command: '', depends_on: ['lint', 'test', 'typecheck'], started_at: Date.now() - 15000 },
    { id: 'e2e', name: 'E2E Tests', status: 'queued', command: '', depends_on: ['build'] },
    { id: 'deploy', name: 'Deploy', status: 'queued', command: '', depends_on: ['build'] },
  ],
};

const themes: Record<string, ThemeConfig> = {
  dark: darkTheme,
  light: lightTheme,
  highContrast: highContrastTheme,
};

export default function App() {
  const graphRef = useRef<WorkflowGraphHandle>(null);
  const [themeName, setThemeName] = useState('dark');
  const [minimap, setMinimap] = useState(false);
  const [selectedNode, setSelectedNode] = useState<string | null>(null);

  const theme: ThemeConfig = {
    ...themes[themeName],
    minimap,
    edge_styles: {
      'build->deploy': { color: '#f97316', width: 3, dash: [6, 3] },
    },
  };

  const handleNodeClick = useCallback((jobId: string) => {
    setSelectedNode(jobId);
    console.log('Clicked:', jobId);
  }, []);

  const handleError = useCallback((err: Error) => {
    console.error('Graph error:', err);
  }, []);

  return (
    <div style={{ padding: 24, fontFamily: 'system-ui', background: '#0d1117', color: '#e6edf3', minHeight: '100vh' }}>
      <h1 style={{ fontSize: 20, marginBottom: 16 }}>workflow-graph — React</h1>

      <div style={{ display: 'flex', gap: 8, marginBottom: 16, flexWrap: 'wrap' }}>
        {Object.keys(themes).map((name) => (
          <button
            key={name}
            onClick={() => setThemeName(name)}
            style={{
              padding: '6px 16px',
              borderRadius: 6,
              border: `1px solid ${themeName === name ? '#58a6ff' : '#30363d'}`,
              background: '#161b22',
              color: themeName === name ? '#58a6ff' : '#e6edf3',
              cursor: 'pointer',
            }}
          >
            {name}
          </button>
        ))}
        <button
          onClick={() => setMinimap((m) => !m)}
          style={{
            padding: '6px 16px',
            borderRadius: 6,
            border: `1px solid ${minimap ? '#58a6ff' : '#30363d'}`,
            background: '#161b22',
            color: minimap ? '#58a6ff' : '#e6edf3',
            cursor: 'pointer',
          }}
        >
          Minimap {minimap ? 'ON' : 'OFF'}
        </button>
        <button
          onClick={() => graphRef.current?.zoomToFit()}
          style={{ padding: '6px 16px', borderRadius: 6, border: '1px solid #30363d', background: '#161b22', color: '#e6edf3', cursor: 'pointer' }}
        >
          Zoom to Fit
        </button>
      </div>

      {selectedNode && (
        <div style={{ marginBottom: 16, padding: 8, background: '#161b22', borderRadius: 6, border: '1px solid #30363d' }}>
          Selected: <strong>{selectedNode}</strong>
        </div>
      )}

      <WorkflowGraphComponent
        ref={graphRef}
        workflow={sampleWorkflow}
        theme={theme}
        autoResize
        onNodeClick={handleNodeClick}
        onEdgeClick={(from: any, to: any) => console.log('Edge:', from, '->', to)}
        onError={handleError}
        style={{ border: '1px solid #30363d', borderRadius: 8, overflow: 'hidden' }}
      />
    </div>
  );
}
