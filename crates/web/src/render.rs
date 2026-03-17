use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

use workflow_graph_shared::{JobStatus, Workflow};

use crate::layout::{GraphLayout, NodeLayout};
use crate::theme;

pub const COLOR_HIGHLIGHT: &str = "#0969da";
pub const COLOR_SELECTED: &str = "#0969da";

#[allow(clippy::too_many_arguments)]
pub fn render(
    ctx: &CanvasRenderingContext2d,
    workflow: &Workflow,
    layout: &GraphLayout,
    dpr: f64,
    highlighted_edges: &[usize],
    tw: f64,
    th: f64,
    animation_time: f64,
    now_ms: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    selected: &HashSet<String>,
) -> Result<(), JsValue> {
    // Clear canvas
    ctx.save();
    ctx.set_fill_style_str(theme::COLOR_BG);
    ctx.fill_rect(0.0, 0.0, tw * dpr, th * dpr);
    ctx.scale(dpr, dpr)?;

    // Apply pan and zoom transform
    ctx.translate(pan_x, pan_y)?;
    ctx.scale(zoom, zoom)?;

    // Draw graph background card
    draw_rounded_rect(
        ctx,
        theme::PADDING - 10.0,
        theme::PADDING - 10.0,
        tw - 2.0 * theme::PADDING + 20.0,
        th - 2.0 * theme::PADDING + 20.0,
        12.0,
    );
    ctx.set_fill_style_str(theme::COLOR_GRAPH_BG);
    ctx.fill();
    ctx.set_stroke_style_str(theme::COLOR_NODE_BORDER);
    ctx.set_line_width(1.0);
    ctx.stroke();

    // Draw header
    draw_header(ctx, workflow);

    // Build node lookup
    let node_map: HashMap<&str, &NodeLayout> = layout
        .nodes
        .iter()
        .map(|n| (n.job_id.as_str(), n))
        .collect();
    let job_map: HashMap<&str, &workflow_graph_shared::Job> =
        workflow.jobs.iter().map(|j| (j.id.as_str(), j)).collect();

    // Draw edges (behind nodes)
    for (i, edge) in layout.edges.iter().enumerate() {
        if let (Some(from), Some(to)) = (
            node_map.get(edge.from_id.as_str()),
            node_map.get(edge.to_id.as_str()),
        ) {
            let highlighted = highlighted_edges.contains(&i);
            draw_edge(ctx, from, to, highlighted);
        }
    }

    // Draw nodes
    for node in &layout.nodes {
        if let Some(job) = job_map.get(node.job_id.as_str()) {
            let is_selected = selected.contains(&node.job_id);
            draw_node(ctx, node, job, animation_time, now_ms, is_selected);
        }
    }

    ctx.restore();
    Ok(())
}

fn draw_header(ctx: &CanvasRenderingContext2d, workflow: &Workflow) {
    let x = theme::PADDING;
    let y = theme::PADDING + 10.0;

    ctx.set_font(&format!(
        "bold {}px {}",
        theme::FONT_SIZE_HEADER,
        theme::FONT_FAMILY
    ));
    ctx.set_fill_style_str(theme::COLOR_HEADER_TEXT);
    ctx.fill_text(&workflow.name, x, y).ok();

    ctx.set_font(&format!(
        "{}px {}",
        theme::FONT_SIZE_HEADER,
        theme::FONT_FAMILY
    ));
    ctx.set_fill_style_str(theme::COLOR_HEADER_TRIGGER);
    let name_width = ctx
        .measure_text(&workflow.name)
        .map(|m| m.width())
        .unwrap_or(60.0);
    ctx.fill_text(&workflow.trigger, x + name_width + 12.0, y)
        .ok();
}

fn draw_edge(
    ctx: &CanvasRenderingContext2d,
    from: &NodeLayout,
    to: &NodeLayout,
    highlighted: bool,
) {
    let x1 = from.x + from.width;
    let y1 = from.y + from.height / 2.0;
    let x2 = to.x;
    let y2 = to.y + to.height / 2.0;

    let mid_x = (x1 + x2) / 2.0;

    let (edge_color, junction_color, line_width) = if highlighted {
        (COLOR_HIGHLIGHT, COLOR_HIGHLIGHT, 2.5)
    } else {
        (theme::COLOR_EDGE, theme::COLOR_JUNCTION, 2.0)
    };

    ctx.begin_path();
    ctx.set_stroke_style_str(edge_color);
    ctx.set_line_width(line_width);
    ctx.move_to(x1, y1);
    ctx.bezier_curve_to(mid_x, y1, mid_x, y2, x2, y2);
    ctx.stroke();

    // Junction dot at source
    ctx.begin_path();
    ctx.set_fill_style_str(junction_color);
    ctx.arc(x1, y1, theme::JUNCTION_DOT_RADIUS, 0.0, 2.0 * PI)
        .ok();
    ctx.fill();

    // Junction dot at target
    ctx.begin_path();
    ctx.arc(x2, y2, theme::JUNCTION_DOT_RADIUS, 0.0, 2.0 * PI)
        .ok();
    ctx.fill();
}

fn draw_node(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    animation_time: f64,
    now_ms: f64,
    is_selected: bool,
) {
    // Node background
    draw_rounded_rect(
        ctx,
        node.x,
        node.y,
        node.width,
        node.height,
        theme::NODE_RADIUS,
    );
    ctx.set_fill_style_str(theme::COLOR_NODE_BG);
    ctx.fill();

    if is_selected {
        ctx.set_stroke_style_str(COLOR_SELECTED);
        ctx.set_line_width(2.0);
    } else {
        ctx.set_stroke_style_str(theme::COLOR_NODE_BORDER);
        ctx.set_line_width(1.0);
    }
    ctx.stroke();

    // Status icon
    let icon_x = node.x + theme::STATUS_ICON_MARGIN + theme::STATUS_ICON_RADIUS;
    let icon_y = node.y + node.height / 2.0;
    draw_status_icon(ctx, icon_x, icon_y, &job.status, animation_time);

    // Job name
    let text_x = icon_x + theme::STATUS_ICON_RADIUS + 8.0;
    let text_y = node.y + node.height / 2.0 + 4.0;
    ctx.set_font(&format!(
        "600 {}px {}",
        theme::FONT_SIZE_NAME,
        theme::FONT_FAMILY
    ));
    ctx.set_fill_style_str(theme::COLOR_TEXT);
    ctx.fill_text(&job.name, text_x, text_y).ok();

    // Duration / live timer (right-aligned)
    let duration_str = match job.status {
        JobStatus::Running => {
            // Live elapsed timer
            if let Some(started) = job.started_at {
                let elapsed_secs = ((now_ms - started) / 1000.0).max(0.0) as u64;
                Some(format_duration(elapsed_secs))
            } else {
                None
            }
        }
        JobStatus::Success | JobStatus::Failure => job.duration_secs.map(format_duration),
        _ => None,
    };

    if let Some(dur_text) = duration_str {
        ctx.set_font(&format!(
            "{}px {}",
            theme::FONT_SIZE_DURATION,
            theme::FONT_FAMILY
        ));
        let color = if job.status == JobStatus::Running {
            theme::COLOR_RUNNING
        } else {
            theme::COLOR_TEXT_SECONDARY
        };
        ctx.set_fill_style_str(color);
        let dur_width = ctx
            .measure_text(&dur_text)
            .map(|m| m.width())
            .unwrap_or(30.0);
        let dur_x = node.x + node.width - dur_width - 10.0;
        ctx.fill_text(&dur_text, dur_x, text_y).ok();
    }
}

// GitHub Octicon SVG path data (16x16 viewBox)
const OCTICON_CHECK_CIRCLE_FILL: &str = "M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-.018-1.042.751.751 0 0 0-1.042-.018L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042l2 2a.75.75 0 0 0 1.06 0Z";
const OCTICON_X_CIRCLE_FILL: &str = "M2.343 13.657A8 8 0 1 1 13.658 2.343 8 8 0 0 1 2.343 13.657ZM6.03 4.97a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042L6.94 8 4.97 9.97a.749.749 0 0 0 .326 1.275.749.749 0 0 0 .734-.215L8 9.06l1.97 1.97a.749.749 0 0 0 1.275-.326.749.749 0 0 0-.215-.734L9.06 8l1.97-1.97a.749.749 0 0 0-.326-1.275.749.749 0 0 0-.734.215L8 6.94Z";
const OCTICON_SKIP_FILL: &str = "M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm11.333-2.167a.825.825 0 0 0-1.166-1.166l-5.5 5.5a.825.825 0 0 0 1.166 1.166Z";
const OCTICON_CIRCLE: &str =
    "M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13Z";

/// Draw an Octicon SVG path scaled and positioned at (cx, cy) with the given radius.
/// The path data is for a 16x16 viewBox, so we scale by (2*radius)/16 and translate.
fn draw_octicon(
    ctx: &CanvasRenderingContext2d,
    cx: f64,
    cy: f64,
    radius: f64,
    path_data: &str,
    color: &str,
) {
    let scale = (2.0 * radius) / 16.0;
    ctx.save();
    // Translate so that the 16x16 icon center (8,8) maps to (cx, cy)
    ctx.translate(cx - 8.0 * scale, cy - 8.0 * scale).ok();
    ctx.scale(scale, scale).ok();

    let path = web_sys::Path2d::new_with_path_string(path_data).unwrap();
    ctx.set_fill_style_str(color);
    ctx.fill_with_path_2d(&path);

    ctx.restore();
}

/// Draw GitHub Octicon status icons using exact SVG path data.
fn draw_status_icon(
    ctx: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    status: &JobStatus,
    animation_time: f64,
) {
    let r = theme::STATUS_ICON_RADIUS;

    match status {
        JobStatus::Queued => {
            // Hollow gray circle — octicon circle-16
            draw_octicon(ctx, x, y, r, OCTICON_CIRCLE, theme::COLOR_QUEUED);
        }
        JobStatus::Running => {
            // Glowing arc ring spinner — gradient fade tail
            let track_r = r - 1.0;
            let line_w = 2.5;

            // Dim track ring
            ctx.begin_path();
            ctx.set_stroke_style_str("rgba(191,135,0,0.15)");
            ctx.set_line_width(line_w);
            ctx.arc(x, y, track_r, 0.0, 2.0 * PI).ok();
            ctx.stroke();

            // Gradient arc: draw segments from tail (faint) to head (bright)
            let total_sweep = PI * 1.2;
            let head_angle = animation_time * 4.0;
            let segments = 20;
            let seg_sweep = total_sweep / segments as f64;
            ctx.save();
            ctx.set_line_cap("butt");
            ctx.set_line_width(line_w);
            for i in 0..segments {
                let t = i as f64 / segments as f64;
                let alpha = t * t; // quadratic ease-in: faint tail, bright head
                let seg_start = head_angle - total_sweep + (i as f64) * seg_sweep;
                ctx.begin_path();
                let color = format!("rgba(191,135,0,{:.2})", alpha);
                ctx.set_stroke_style_str(&color);
                // Slight overlap to avoid gaps
                ctx.arc(x, y, track_r, seg_start, seg_start + seg_sweep + 0.02)
                    .ok();
                ctx.stroke();
            }

            // Bright head cap
            ctx.begin_path();
            ctx.set_stroke_style_str(theme::COLOR_RUNNING);
            ctx.set_line_width(line_w);
            ctx.set_line_cap("round");
            ctx.arc(x, y, track_r, head_angle - seg_sweep, head_angle)
                .ok();
            ctx.stroke();

            ctx.restore();
        }
        JobStatus::Success => {
            // Green check-circle-fill — exact GitHub octicon
            draw_octicon(
                ctx,
                x,
                y,
                r,
                OCTICON_CHECK_CIRCLE_FILL,
                theme::COLOR_SUCCESS,
            );
        }
        JobStatus::Failure => {
            // Red x-circle-fill — exact GitHub octicon
            draw_octicon(ctx, x, y, r, OCTICON_X_CIRCLE_FILL, theme::COLOR_FAILURE);
        }
        JobStatus::Skipped => {
            // Gray skip-fill — exact GitHub octicon (diagonal line through circle)
            draw_octicon(ctx, x, y, r, OCTICON_SKIP_FILL, theme::COLOR_SKIPPED);
        }
        JobStatus::Cancelled => {
            // Gray circle with stop indicator
            draw_octicon(ctx, x, y, r, OCTICON_SKIP_FILL, theme::COLOR_CANCELLED);
        }
    }
}

fn draw_rounded_rect(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64) {
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    ctx.arc_to(x + w, y, x + w, y + r, r).ok();
    ctx.line_to(x + w, y + h - r);
    ctx.arc_to(x + w, y + h, x + w - r, y + h, r).ok();
    ctx.line_to(x + r, y + h);
    ctx.arc_to(x, y + h, x, y + h - r, r).ok();
    ctx.line_to(x, y + r);
    ctx.arc_to(x, y, x + r, y, r).ok();
    ctx.close_path();
}

fn format_duration(secs: u64) -> String {
    if secs >= 60 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{m}m {s}s")
    } else {
        format!("{secs}s")
    }
}
