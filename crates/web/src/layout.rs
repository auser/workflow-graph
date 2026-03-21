use std::collections::HashMap;
use workflow_graph_shared::{NodeDefinition, Workflow};

use crate::theme::{LayoutDirection, ResolvedTheme};

/// Compute the height for a node based on its NodeDefinition (if any).
/// Returns theme default if no definition is registered.
fn compute_node_height(
    job: &workflow_graph_shared::Job,
    node_defs: &HashMap<String, NodeDefinition>,
    default_height: f64,
) -> f64 {
    let node_type = job.metadata.get("node_type").and_then(|v| v.as_str());
    let def = node_type.and_then(|t| node_defs.get(t));
    match def {
        Some(d) if !d.fields.is_empty() => {
            // header(28) + name line(24) + fields + ports + padding
            let field_h = d.fields.len() as f64 * 22.0;
            let port_count = job.ports.len().max(d.inputs.len() + d.outputs.len());
            let port_h = port_count as f64 * 20.0;
            let computed = 28.0 + 24.0 + field_h + port_h.max(0.0) + 12.0;
            computed.max(default_height)
        }
        Some(_) => {
            // Has a definition but no fields — use header + name + ports
            let port_count = job.ports.len();
            let computed = 28.0 + 24.0 + (port_count as f64 * 20.0).max(20.0) + 12.0;
            computed.max(default_height)
        }
        None => default_height,
    }
}

#[derive(Clone, Debug)]
pub struct NodeLayout {
    pub job_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    /// Source port id (empty = legacy node-to-node edge).
    pub from_port: String,
    /// Target port id (empty = legacy node-to-node edge).
    pub to_port: String,
    /// Arbitrary metadata for custom edge rendering (e.g., condition labels, edge type).
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct GraphLayout {
    pub nodes: Vec<NodeLayout>,
    pub edges: Vec<Edge>,
    pub total_width: f64,
    pub total_height: f64,
}

pub fn compute_layout(workflow: &Workflow, theme: &ResolvedTheme) -> GraphLayout {
    compute_layout_with_defs(workflow, theme, &HashMap::new())
}

pub fn compute_layout_with_defs(
    workflow: &Workflow,
    theme: &ResolvedTheme,
    node_defs: &HashMap<String, NodeDefinition>,
) -> GraphLayout {
    let jobs = &workflow.jobs;
    if jobs.is_empty() {
        return GraphLayout {
            nodes: vec![],
            edges: vec![],
            total_width: 0.0,
            total_height: 0.0,
        };
    }

    let tl = &theme.layout;
    let is_vertical = theme.direction == LayoutDirection::TopToBottom;

    // Build adjacency: job_id -> index
    let id_to_idx: HashMap<&str, usize> = jobs
        .iter()
        .enumerate()
        .map(|(i, j)| (j.id.as_str(), i))
        .collect();

    // Layer assignment: column = 1 + max(column of deps)
    let mut layers: Vec<usize> = vec![0; jobs.len()];
    let mut visited = vec![false; jobs.len()];

    fn assign_layer(
        idx: usize,
        jobs: &[workflow_graph_shared::Job],
        id_to_idx: &HashMap<&str, usize>,
        layers: &mut Vec<usize>,
        visited: &mut Vec<bool>,
    ) -> usize {
        if visited[idx] {
            return layers[idx];
        }
        visited[idx] = true;
        let mut max_dep_layer = 0;
        let has_deps = !jobs[idx].depends_on.is_empty();
        for dep_id in &jobs[idx].depends_on {
            if let Some(&dep_idx) = id_to_idx.get(dep_id.as_str()) {
                let dep_layer = assign_layer(dep_idx, jobs, id_to_idx, layers, visited);
                max_dep_layer = max_dep_layer.max(dep_layer + 1);
            }
        }
        layers[idx] = if has_deps { max_dep_layer } else { 0 };
        layers[idx]
    }

    for i in 0..jobs.len() {
        assign_layer(i, jobs, &id_to_idx, &mut layers, &mut visited);
    }

    // Group jobs by layer
    let max_layer = *layers.iter().max().unwrap_or(&0);
    let mut layer_groups: Vec<Vec<usize>> = vec![vec![]; max_layer + 1];
    for (i, &layer) in layers.iter().enumerate() {
        layer_groups[layer].push(i);
    }

    // Barycenter ordering: sort each layer by avg position of upstream deps
    let mut positions: Vec<(usize, f64)> = vec![(0, 0.0); jobs.len()];

    for (rank, &idx) in layer_groups[0].iter().enumerate() {
        positions[idx] = (rank, rank as f64);
    }

    for group in layer_groups.iter_mut().skip(1) {
        let mut barycenters: Vec<(usize, f64)> = group
            .iter()
            .map(|&idx| {
                let deps: Vec<f64> = jobs[idx]
                    .depends_on
                    .iter()
                    .filter_map(|dep_id| id_to_idx.get(dep_id.as_str()))
                    .map(|&dep_idx| positions[dep_idx].1)
                    .collect();
                let avg = if deps.is_empty() {
                    0.0
                } else {
                    deps.iter().sum::<f64>() / deps.len() as f64
                };
                (idx, avg)
            })
            .collect();

        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        *group = barycenters.iter().map(|&(idx, _)| idx).collect();
        for (rank, &idx) in group.iter().enumerate() {
            positions[idx] = (rank, rank as f64);
        }
    }

    // Convert to pixel coordinates
    let mut nodes = Vec::with_capacity(jobs.len());
    let mut max_x: f64 = 0.0;
    let mut max_y: f64 = 0.0;

    // Pre-compute per-node heights
    let node_heights: Vec<f64> = jobs
        .iter()
        .map(|j| compute_node_height(j, node_defs, tl.node_height))
        .collect();

    // Compute cumulative layer offsets (for variable heights)
    let mut layer_y_offsets: Vec<f64> = Vec::with_capacity(layer_groups.len());
    let mut cumulative_y = tl.padding + tl.header_height;
    for group in &layer_groups {
        layer_y_offsets.push(cumulative_y);
        // Max height in this layer determines the next layer's offset
        let max_h = group
            .iter()
            .map(|&idx| node_heights[idx])
            .fold(tl.node_height, f64::max);
        cumulative_y += max_h + if is_vertical { tl.h_gap } else { tl.v_gap };
    }

    for (layer, group) in layer_groups.iter().enumerate() {
        for (rank, &idx) in group.iter().enumerate() {
            let h = node_heights[idx];
            let (x, y) = if is_vertical {
                let x = tl.padding + rank as f64 * (tl.node_width + tl.v_gap);
                let y = layer_y_offsets[layer];
                (x, y)
            } else {
                let x = tl.padding + layer as f64 * (tl.node_width + tl.h_gap);
                let y = layer_y_offsets[layer] + rank as f64 * (tl.node_height + tl.v_gap);
                (x, y)
            };
            nodes.push(NodeLayout {
                job_id: jobs[idx].id.clone(),
                x,
                y,
                width: tl.node_width,
                height: h,
            });
            max_x = max_x.max(x + tl.node_width);
            max_y = max_y.max(y + h);
        }
    }

    // Build node lookup
    let node_map: HashMap<&str, &NodeLayout> =
        nodes.iter().map(|n| (n.job_id.as_str(), n)).collect();

    // Build edges
    let mut edges = Vec::new();
    for job in jobs {
        for dep_id in &job.depends_on {
            if node_map.contains_key(dep_id.as_str()) {
                edges.push(Edge {
                    from_id: dep_id.clone(),
                    to_id: job.id.clone(),
                    from_port: String::new(),
                    to_port: String::new(),
                    metadata: HashMap::new(),
                });
            }
        }
    }

    GraphLayout {
        nodes,
        edges,
        total_width: max_x + tl.padding,
        total_height: max_y + tl.padding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ResolvedTheme;
    use workflow_graph_shared::{Job, JobStatus, Workflow};

    fn make_job(id: &str, depends_on: Vec<String>) -> Job {
        Job {
            id: id.into(),
            name: id.into(),
            status: JobStatus::Queued,
            command: "echo test".into(),
            duration_secs: None,
            started_at: None,
            depends_on,
            output: None,
            required_labels: vec![],
            max_retries: 0,
            attempt: 0,
            metadata: HashMap::new(),
            ports: vec![],
            children: None,
            collapsed: false,
        }
    }

    fn make_workflow(jobs: Vec<Job>) -> Workflow {
        Workflow {
            id: "test".into(),
            name: "Test".into(),
            trigger: "on: push".into(),
            jobs,
        }
    }

    #[test]
    fn compute_layout_empty_workflow() {
        let wf = make_workflow(vec![]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert!(layout.nodes.is_empty());
        assert!(layout.edges.is_empty());
        assert_eq!(layout.total_width, 0.0);
        assert_eq!(layout.total_height, 0.0);
    }

    #[test]
    fn compute_layout_single_node() {
        let wf = make_workflow(vec![make_job("a", vec![])]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert_eq!(layout.nodes.len(), 1);
        assert_eq!(layout.nodes[0].job_id, "a");
        assert!(layout.edges.is_empty());
    }

    #[test]
    fn compute_layout_creates_edges_with_empty_metadata() {
        let wf = make_workflow(vec![make_job("a", vec![]), make_job("b", vec!["a".into()])]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert_eq!(layout.edges.len(), 1);
        assert_eq!(layout.edges[0].from_id, "a");
        assert_eq!(layout.edges[0].to_id, "b");
        assert!(layout.edges[0].metadata.is_empty());
    }

    #[test]
    fn compute_layout_multiple_edges() {
        let wf = make_workflow(vec![
            make_job("a", vec![]),
            make_job("b", vec![]),
            make_job("c", vec!["a".into(), "b".into()]),
        ]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert_eq!(layout.nodes.len(), 3);
        assert_eq!(layout.edges.len(), 2);

        let edge_pairs: Vec<(&str, &str)> = layout
            .edges
            .iter()
            .map(|e| (e.from_id.as_str(), e.to_id.as_str()))
            .collect();
        assert!(edge_pairs.contains(&("a", "c")));
        assert!(edge_pairs.contains(&("b", "c")));
    }

    #[test]
    fn compute_layout_diamond_dag() {
        // a -> b, a -> c, b -> d, c -> d
        let wf = make_workflow(vec![
            make_job("a", vec![]),
            make_job("b", vec!["a".into()]),
            make_job("c", vec!["a".into()]),
            make_job("d", vec!["b".into(), "c".into()]),
        ]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert_eq!(layout.nodes.len(), 4);
        assert_eq!(layout.edges.len(), 4);
    }

    #[test]
    fn compute_layout_assigns_different_layers() {
        let wf = make_workflow(vec![
            make_job("a", vec![]),
            make_job("b", vec!["a".into()]),
            make_job("c", vec!["b".into()]),
        ]);
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        // In left-to-right layout, nodes in later layers should have larger x values
        let node_a = layout.nodes.iter().find(|n| n.job_id == "a").unwrap();
        let node_b = layout.nodes.iter().find(|n| n.job_id == "b").unwrap();
        let node_c = layout.nodes.iter().find(|n| n.job_id == "c").unwrap();

        assert!(node_a.x < node_b.x, "a should be left of b");
        assert!(node_b.x < node_c.x, "b should be left of c");
    }

    #[test]
    fn edge_struct_holds_metadata() {
        let mut meta = HashMap::new();
        meta.insert("label".into(), serde_json::json!("on success"));
        meta.insert("style".into(), serde_json::json!("dashed"));

        let edge = Edge {
            from_id: "a".into(),
            to_id: "b".into(),
            from_port: String::new(),
            to_port: String::new(),
            metadata: meta,
        };

        assert_eq!(edge.metadata.len(), 2);
        assert_eq!(edge.metadata["label"], serde_json::json!("on success"));
        assert_eq!(edge.metadata["style"], serde_json::json!("dashed"));
    }

    #[test]
    fn compute_layout_sample_workflow() {
        let wf = Workflow::sample();
        let theme = ResolvedTheme::default();
        let layout = compute_layout(&wf, &theme);

        assert_eq!(layout.nodes.len(), wf.jobs.len());
        // Sample workflow has edges: 3 roots -> build, build -> 3 downstream, deploy-db -> deploy-web
        assert!(!layout.edges.is_empty());
        assert!(layout.total_width > 0.0);
        assert!(layout.total_height > 0.0);

        // All edges should have empty metadata (from compute_layout)
        for edge in &layout.edges {
            assert!(edge.metadata.is_empty());
        }
    }
}
