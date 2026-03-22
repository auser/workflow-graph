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
  type GraphState as GraphStateType,
  type GraphOptions,
  type ThemeConfig,
  type NodeDefinition as NodeDefinitionType,
} from '@auser/workflow-graph-web';

export type {
  Workflow,
  Job,
  Port,
  PortDirection,
  EdgeInfo,
  GraphState,
  GraphOptions,
  PersistOptions,
  PersistStorage,
  ThemeConfig,
  ThemeColors,
  ThemeFonts,
  ThemeLayout,
  LayoutDirection,
  EdgeStyle,
  Labels,
  OnRenderNode,
  NodeRenderer,
  NodeDefinition,
  FieldDef,
  FieldType,
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
  addNode(job: JobType, x?: number, y?: number): Promise<void>;
  removeNode(jobId: string): Promise<void>;
  updateNode(jobId: string, partial: Partial<JobType>): Promise<void>;
  addEdge(fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>): Promise<void>;
  removeEdge(fromId: string, toId: string): Promise<void>;
  getNodes(): Promise<JobType[]>;
  getEdges(): Promise<EdgeInfoType[]>;
  getState(): Promise<GraphStateType | null>;
  loadState(state: GraphStateType): Promise<void>;
  groupSelected(groupName?: string): Promise<void>;
  ungroupNode(nodeId: string): Promise<void>;
  toggleCollapse(nodeId: string): Promise<void>;
  registerNodeType(def: NodeDefinitionType): void;
  registerNodeTypes(defs: NodeDefinitionType[]): void;
  getNodeType(type: string): NodeDefinitionType | null;
  getNodeTypes(): NodeDefinitionType[];
  canvasToScreen(x: number, y: number): { x: number; y: number };
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
  /** Initial node positions to restore immediately after init. */
  initialPositions?: Record<string, [number, number]>;
  /** Node type definitions — auto-registered on mount. */
  nodeTypes?: NodeDefinitionType[];
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
      persist,
      onFieldClick,
      onError,
      loadingSkeleton,
      initialPositions,
      nodeTypes,
    },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null);
    const graphRef = useRef<WorkflowGraph | null>(null);
    const workflowRef = useRef<Workflow>(workflow);
    const destroyedRef = useRef(false);
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
        addNode: (job: JobType, x?: number, y?: number) => graphRef.current?.addNode(job, x, y) ?? Promise.resolve(),
        removeNode: (jobId: string) => graphRef.current?.removeNode(jobId) ?? Promise.resolve(),
        updateNode: (jobId: string, partial: Partial<JobType>) =>
          graphRef.current?.updateNode(jobId, partial) ?? Promise.resolve(),
        addEdge: (fromId: string, toId: string, fromPort?: string, toPort?: string, metadata?: Record<string, unknown>) =>
          graphRef.current?.addEdge(fromId, toId, fromPort, toPort, metadata) ?? Promise.resolve(),
        removeEdge: (fromId: string, toId: string) =>
          graphRef.current?.removeEdge(fromId, toId) ?? Promise.resolve(),
        getNodes: () => graphRef.current?.getNodes() ?? Promise.resolve([] as JobType[]),
        getEdges: () => graphRef.current?.getEdges() ?? Promise.resolve([] as EdgeInfoType[]),
        getState: () => graphRef.current?.getState() ?? Promise.resolve(null),
        loadState: (state: GraphStateType) => graphRef.current?.loadState(state) ?? Promise.resolve(),
        groupSelected: (groupName?: string) => graphRef.current?.groupSelected(groupName) ?? Promise.resolve(),
        ungroupNode: (nodeId: string) => graphRef.current?.ungroupNode(nodeId) ?? Promise.resolve(),
        toggleCollapse: (nodeId: string) => graphRef.current?.toggleCollapse(nodeId) ?? Promise.resolve(),
        registerNodeType: (def: NodeDefinitionType) => graphRef.current?.registerNodeType(def),
        registerNodeTypes: (defs: NodeDefinitionType[]) => graphRef.current?.registerNodeTypes(defs),
        getNodeType: (type: string) => graphRef.current?.getNodeType(type) ?? null,
        getNodeTypes: () => graphRef.current?.getNodeTypes() ?? [],
        canvasToScreen: (x: number, y: number) => graphRef.current?.canvasToScreen(x, y) ?? { x: 0, y: 0 },
        get instance() {
          return graphRef.current;
        },
      }),
      [],
    );

    // Initialize graph on mount
    useEffect(() => {
      if (typeof document === 'undefined' || !containerRef.current) return;

      // If a previous graph exists (React StrictMode re-mount), destroy it
      // synchronously first to prevent stale WASM state corruption.
      if (graphRef.current) {
        graphRef.current.destroy().catch(() => {});
        graphRef.current = null;
      }
      destroyedRef.current = false;

      // Clear any leftover canvas elements from previous mount
      const container = containerRef.current;
      while (container.querySelector('canvas')) {
        container.querySelector('canvas')!.remove();
      }

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
        persist,
        onFieldClick,
      };

      const graph = new WorkflowGraph(container, options);
      graphRef.current = graph;

      // Register node type definitions
      if (nodeTypes && nodeTypes.length > 0) {
        graph.registerNodeTypes(nodeTypes);
      }

      graph
        .setWorkflow(workflow)
        .then(async () => {
          if (!destroyedRef.current) {
            // Re-apply theme after init to ensure it takes effect
            if (theme) {
              await graph.setTheme(theme).catch(() => {});
            }
            // Persisted state is auto-restored inside setWorkflow when persist is configured.
            // Override with explicit initialPositions if provided
            if (initialPositions && Object.keys(initialPositions).length > 0) {
              await graph.setNodePositions(initialPositions).catch(() => {});
            }
            setLoading(false);
            setError(null);
          }
        })
        .catch((err: unknown) => {
          if (!destroyedRef.current) {
            const e = err instanceof Error ? err : new Error(String(err));
            setError(e);
            setLoading(false);
            onError?.(e);
          }
        });

      return () => {
        destroyedRef.current = true;
        // Remove the canvas from DOM immediately so mouse/keyboard events
        // can't fire on it during the async destroy gap.
        // The graph.destroy() will handle WASM cleanup after.
        const canvases = container.querySelectorAll('canvas');
        canvases.forEach((c) => c.remove());
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

        // Re-apply saved positions after topology update
        if (initialPositions && Object.keys(initialPositions).length > 0) {
          graphRef.current.setNodePositions(initialPositions).catch(() => {});
        }
      }
    }, [workflow, onError, loading, initialPositions]);

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
      <div className={className} style={{ width: '100%', height: '100%', ...style, position: 'relative' }}>
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
