use std::collections::HashMap;
use workflow_graph_shared::Workflow;

use crate::theme::{LayoutDirection, ResolvedTheme};

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
}

#[derive(Clone, Debug)]
pub struct GraphLayout {
    pub nodes: Vec<NodeLayout>,
    pub edges: Vec<Edge>,
    pub total_width: f64,
    pub total_height: f64,
}

pub fn compute_layout(workflow: &Workflow, theme: &ResolvedTheme) -> GraphLayout {
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

    for (layer, group) in layer_groups.iter().enumerate() {
        for (rank, &idx) in group.iter().enumerate() {
            let (x, y) = if is_vertical {
                // Top-to-bottom: layers go down, siblings go right
                let x = tl.padding + rank as f64 * (tl.node_width + tl.v_gap);
                let y = tl.padding + tl.header_height + layer as f64 * (tl.node_height + tl.h_gap);
                (x, y)
            } else {
                // Left-to-right (default): layers go right, siblings go down
                let x = tl.padding + layer as f64 * (tl.node_width + tl.h_gap);
                let y = tl.padding + tl.header_height + rank as f64 * (tl.node_height + tl.v_gap);
                (x, y)
            };
            nodes.push(NodeLayout {
                job_id: jobs[idx].id.clone(),
                x,
                y,
                width: tl.node_width,
                height: tl.node_height,
            });
            max_x = max_x.max(x + tl.node_width);
            max_y = max_y.max(y + tl.node_height);
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
