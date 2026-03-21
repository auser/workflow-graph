/**
 * @auser/workflow-graph-react — React component for workflow DAG visualization
 */

import React, {
  useEffect,
  useRef,
  useImperativeHandle,
  forwardRef,
  useState,
} from 'react';
import {
  WorkflowGraph,
  type Workflow,
  type Job as JobType,
  type EdgeInfo as EdgeInfoType,
  type GraphOptions,
  type ThemeConfig,
} from '@auser/workflow-graph-web';

export type {
  Workflow,
  Job,
  Port,
  PortDirection,
  EdgeInfo,
  GraphOptions,
  ThemeConfig,
  ThemeColors,
  ThemeFonts,
  ThemeLayout,
  LayoutDirection,
  EdgeStyle,
  Labels,
  OnRenderNode,
} from '@auser/workflow-graph-web';
export { darkTheme, lightTheme, highContrastTheme } from '@auser/workflow-graph-web';

export interface WorkflowGraphHandle {
  selectNode(jobId: string): Promise<void>;
  deselectAll(): Promise<void>;
  resetLayout(): Promise<void>;
  zoomToFit(): Promise<void>;
  setZoom(level: number): Promise<void>;
  getNodePositions(): Promise<Record<string, [number, number]>>;
  setNodePositions(positions: Record<string, [number, number]>): Promise<void>;
  setTheme(theme: ThemeConfig): Promise<void>;
  addNode(job: JobType): Promise<void>;
  removeNode(jobId: string): Promise<void>;
  updateNode(jobId: string, partial: Partial<JobType>): Promise<void>;
  addEdge(fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>): Promise<void>;
  removeEdge(fromId: string, toId: string): Promise<void>;
  getNodes(): Promise<JobType[]>;
  getEdges(): Promise<EdgeInfoType[]>;
  readonly instance: WorkflowGraph | null;
}

export interface WorkflowGraphProps extends GraphOptions {
  workflow: Workflow;
  className?: string;
  style?: React.CSSProperties;
  /** Called when the WASM module fails to load or render. */
  onError?: (error: Error) => void;
  /** Custom loading skeleton. Defaults to a pulsing placeholder. */
  loadingSkeleton?: React.ReactNode;
}

// Default loading skeleton
const DefaultSkeleton: React.FC = () => (
  <div
    style={{
      width: '100%',
      minHeight: 120,
      borderRadius: 8,
      background: 'linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%)',
      backgroundSize: '200% 100%',
      animation: 'wg-skeleton-pulse 1.5s ease-in-out infinite',
    }}
    role="progressbar"
    aria-label="Loading workflow graph"
  >
    <style>{`
      @keyframes wg-skeleton-pulse {
        0% { background-position: 200% 0; }
        100% { background-position: -200% 0; }
      }
    `}</style>
  </div>
);

/**
 * React component that renders an interactive workflow DAG.
 *
 * Supports `ref` for imperative control (select, zoom, theme changes).
 *
 * @example
 * ```tsx
 * const ref = useRef<WorkflowGraphHandle>(null);
 *
 * <WorkflowGraphComponent
 *   ref={ref}
 *   workflow={workflowData}
 *   theme={darkTheme}
 *   autoResize
 *   onNodeClick={(id) => console.log('clicked', id)}
 *   onError={(err) => console.error('Graph error:', err)}
 * />
 *
 * // Imperative API
 * ref.current?.zoomToFit();
 * ref.current?.setTheme(lightTheme);
 * ```
 */
export const WorkflowGraphComponent = forwardRef<WorkflowGraphHandle, WorkflowGraphProps>(
  function WorkflowGraphComponent(
    {
      workflow,
      className,
      style,
      onNodeClick,
      onNodeHover,
      onCanvasClick,
      onSelectionChange,
      onNodeDragEnd,
      onEdgeClick,
      onRenderNode,
      onDrop,
      onConnect,
      theme,
      autoResize,
      onError,
      loadingSkeleton,
    },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null);
    const graphRef = useRef<WorkflowGraph | null>(null);
    const workflowRef = useRef<Workflow>(workflow);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<Error | null>(null);

    // Expose imperative handle
    useImperativeHandle(
      ref,
      () => ({
        selectNode: (jobId: string) => graphRef.current?.selectNode(jobId) ?? Promise.resolve(),
        deselectAll: () => graphRef.current?.deselectAll() ?? Promise.resolve(),
        resetLayout: () => graphRef.current?.resetLayout() ?? Promise.resolve(),
        zoomToFit: () => graphRef.current?.zoomToFit() ?? Promise.resolve(),
        setZoom: (level: number) => graphRef.current?.setZoom(level) ?? Promise.resolve(),
        getNodePositions: () =>
          graphRef.current?.getNodePositions() ?? Promise.resolve({} as Record<string, [number, number]>),
        setNodePositions: (positions: Record<string, [number, number]>) =>
          graphRef.current?.setNodePositions(positions) ?? Promise.resolve(),
        setTheme: (t: ThemeConfig) => graphRef.current?.setTheme(t) ?? Promise.resolve(),
        addNode: (job: JobType) => graphRef.current?.addNode(job) ?? Promise.resolve(),
        removeNode: (jobId: string) => graphRef.current?.removeNode(jobId) ?? Promise.resolve(),
        updateNode: (jobId: string, partial: Partial<JobType>) =>
          graphRef.current?.updateNode(jobId, partial) ?? Promise.resolve(),
        addEdge: (fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>) =>
          graphRef.current?.addEdge(fromId, toId, fromPort, toPort, metadata) ?? Promise.resolve(),
        removeEdge: (fromId: string, toId: string) =>
          graphRef.current?.removeEdge(fromId, toId) ?? Promise.resolve(),
        getNodes: () => graphRef.current?.getNodes() ?? Promise.resolve([] as JobType[]),
        getEdges: () => graphRef.current?.getEdges() ?? Promise.resolve([] as EdgeInfoType[]),
        get instance() {
          return graphRef.current;
        },
      }),
      [],
    );

    // Initialize graph on mount
    useEffect(() => {
      if (typeof document === 'undefined' || !containerRef.current) return;

      const options: GraphOptions = {
        onNodeClick,
        onNodeHover,
        onCanvasClick,
        onSelectionChange,
        onNodeDragEnd,
        onEdgeClick,
        onRenderNode,
        onDrop,
        onConnect,
        theme,
        autoResize,
      };

      const graph = new WorkflowGraph(containerRef.current, options);
      graphRef.current = graph;

      // Defer initialization to next frame to ensure canvas is in the DOM
      requestAnimationFrame(() => {
        graph
          .setWorkflow(workflow)
          .then(() => {
            setLoading(false);
            setError(null);
          })
          .catch((err: unknown) => {
            const e = err instanceof Error ? err : new Error(String(err));
            setError(e);
            setLoading(false);
            onError?.(e);
          });
      });

      return () => {
        graph.destroy().catch(() => {});
        graphRef.current = null;
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Update data when workflow changes (only after initial load completes)
    useEffect(() => {
      if (!loading && graphRef.current && workflow !== workflowRef.current) {
        const prevIds = new Set(workflowRef.current?.jobs?.map((j) => j.id) ?? []);
        const newJobs = workflow?.jobs?.filter((j) => !prevIds.has(j.id)) ?? [];
        workflowRef.current = workflow;

        // Update existing nodes' status
        graphRef.current.updateStatus(workflow).catch((err: unknown) => {
          const e = err instanceof Error ? err : new Error(String(err));
          onError?.(e);
        });

        // Add any new nodes that weren't in the previous workflow
        for (const job of newJobs) {
          graphRef.current.addNode(job).catch(() => {
            // Ignore — node may already exist in WASM state
          });
        }
      }
    }, [workflow, onError, loading]);

    // Update theme when it changes
    useEffect(() => {
      if (graphRef.current && theme) {
        graphRef.current.setTheme(theme).catch((err: unknown) => {
          const e = err instanceof Error ? err : new Error(String(err));
          onError?.(e);
        });
      }
    }, [theme, onError]);

    return (
      <div className={className} style={{ ...style, position: 'relative' }}>
        {(loading || error) && (loadingSkeleton ?? <DefaultSkeleton />)}
        <div
          ref={containerRef}
          style={{
            width: '100%',
            height: '100%',
            ...(loading ? { position: 'absolute', opacity: 0, pointerEvents: 'none' } : {}),
          }}
        />
      </div>
    );
  },
);

export default WorkflowGraphComponent;
