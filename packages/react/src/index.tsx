/**
 * @workflow-graph/react — React component for workflow DAG visualization
 */

import React, { useEffect, useRef, useCallback } from 'react';
import { WorkflowGraph, type Workflow, type GraphOptions } from '@workflow-graph/web';

export type { Workflow, Job, GraphOptions } from '@workflow-graph/web';

export interface WorkflowGraphProps extends GraphOptions {
  workflow: Workflow;
  className?: string;
  style?: React.CSSProperties;
}

/**
 * React component that renders an interactive workflow DAG.
 *
 * @example
 * ```tsx
 * <WorkflowGraphComponent
 *   workflow={workflowData}
 *   onNodeClick={(id) => console.log('clicked', id)}
 * />
 * ```
 */
export function WorkflowGraphComponent({
  workflow,
  className,
  style,
  onNodeClick,
  onNodeHover,
  onCanvasClick,
  onSelectionChange,
  onNodeDragEnd,
}: WorkflowGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<WorkflowGraph | null>(null);
  const workflowRef = useRef<Workflow>(workflow);

  const options: GraphOptions = {
    onNodeClick,
    onNodeHover,
    onCanvasClick,
    onSelectionChange,
    onNodeDragEnd,
  };

  // Initialize graph on mount
  useEffect(() => {
    if (!containerRef.current) return;

    const graph = new WorkflowGraph(containerRef.current, options);
    graphRef.current = graph;
    graph.setWorkflow(workflow);

    return () => {
      graph.destroy();
      graphRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Update data when workflow changes
  useEffect(() => {
    if (graphRef.current && workflow !== workflowRef.current) {
      workflowRef.current = workflow;
      graphRef.current.updateStatus(workflow);
    }
  }, [workflow]);

  return (
    <div
      ref={containerRef}
      className={className}
      style={style}
    />
  );
}

export default WorkflowGraphComponent;
