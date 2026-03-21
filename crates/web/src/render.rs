use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

use workflow_graph_shared::{JobStatus, NodeDefinition, PortDirection, Workflow};

use crate::layout::{GraphLayout, NodeLayout};
use crate::theme::{EdgeStyle, LayoutDirection, ResolvedTheme};

/// Optional callbacks that influence rendering.
pub struct RenderCallbacks<'a> {
    pub on_render_node: Option<&'a js_sys::Function>,
    pub node_definitions: &'a HashMap<String, NodeDefinition>,
    pub selection_rect: Option<(f64, f64, f64, f64)>,
}

/// Active port connection drag state for rendering the preview line.
pub struct PortDragRender {
    pub from_x: f64,
    pub from_y: f64,
    pub to_x: f64,
    pub to_y: f64,
    pub color: String,
}

#[allow(dead_code)]
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
    theme: &ResolvedTheme,
) -> Result<(), JsValue> {
    render_with_callbacks(
        ctx,
        workflow,
        layout,
        dpr,
        highlighted_edges,
        tw,
        th,
        animation_time,
        now_ms,
        zoom,
        pan_x,
        pan_y,
        selected,
        theme,
        &RenderCallbacks {
            on_render_node: None,
            node_definitions: &HashMap::new(),
            selection_rect: None,
        },
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_with_callbacks(
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
    theme: &ResolvedTheme,
    callbacks: &RenderCallbacks,
    port_drag: Option<&PortDragRender>,
) -> Result<(), JsValue> {
    let colors = &theme.colors;
    let tl = &theme.layout;

    // Clear canvas
    ctx.save();
    ctx.set_fill_style_str(&colors.bg);
    ctx.fill_rect(0.0, 0.0, tw * dpr, th * dpr);
    ctx.scale(dpr, dpr)?;

    // Apply pan and zoom transform
    ctx.translate(pan_x, pan_y)?;
    ctx.scale(zoom, zoom)?;

    // Draw header (skip the constrained background box — nodes can go anywhere)
    if tl.header_height > 0.0 {
        draw_header(ctx, workflow, theme);
    }

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
            let edge_key = format!("{}->{}", edge.from_id, edge.to_id);
            let style_override = theme.edge_styles.get(&edge_key);

            // If edge has port info, draw from/to port positions
            if !edge.from_port.is_empty() && !edge.to_port.is_empty() {
                let from_job = job_map.get(edge.from_id.as_str());
                let to_job = job_map.get(edge.to_id.as_str());
                if let (Some(fj), Some(tj)) = (from_job, to_job) {
                    let from_ports: Vec<_> = fj.ports.iter()
                        .filter(|p| p.direction == PortDirection::Output)
                        .collect();
                    let to_ports: Vec<_> = tj.ports.iter()
                        .filter(|p| p.direction == PortDirection::Input)
                        .collect();
                    let from_idx = from_ports.iter().position(|p| p.id == edge.from_port);
                    let to_idx = to_ports.iter().position(|p| p.id == edge.to_port);
                    if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
                        let fx = from.x + from.width;
                        let fy = from.y + port_y_offset_render(fi, from_ports.len(), from.height);
                        let tx = to.x;
                        let ty = to.y + port_y_offset_render(ti, to_ports.len(), to.height);
                        // Use source port type color for the edge if no explicit style override
                        let port_color_override = if style_override.is_none() {
                            let color = port_type_color(&from_ports[fi].port_type);
                            Some(EdgeStyle {
                                color: Some(color.to_string()),
                                width: Some(2.5),
                                dash: None,
                            })
                        } else {
                            None
                        };
                        let effective_override = style_override.or(port_color_override.as_ref());
                        draw_port_edge(ctx, fx, fy, tx, ty, highlighted, theme, effective_override);
                        continue;
                    }
                }
            }
            draw_edge(ctx, from, to, highlighted, theme, style_override);
        }
    }

    // Draw nodes
    for node in &layout.nodes {
        if let Some(job) = job_map.get(node.job_id.as_str()) {
            let is_selected = selected.contains(&node.job_id);

            // Call custom render callback if provided
            let mut skip_default = false;
            if let Some(cb) = callbacks.on_render_node {
                let job_json = serde_wasm_bindgen::to_value(job).unwrap_or(JsValue::NULL);
                if let Ok(result) = cb.apply(
                    &JsValue::NULL,
                    &js_sys::Array::of5(
                        &JsValue::from_f64(node.x),
                        &JsValue::from_f64(node.y),
                        &JsValue::from_f64(node.width),
                        &JsValue::from_f64(node.height),
                        &job_json,
                    ),
                ) {
                    skip_default = result.as_bool().unwrap_or(false);
                }
            }

            if !skip_default {
                // Look up node type definition from metadata
                let node_type = job.metadata.get("node_type")
                    .and_then(|v| v.as_str());
                let def = node_type.and_then(|t| callbacks.node_definitions.get(t));

                if let Some(def) = def {
                    draw_production_node(ctx, node, job, def, animation_time, now_ms, is_selected, theme);
                } else {
                    draw_node(ctx, node, job, animation_time, now_ms, is_selected, theme);
                }

                // Draw compound node decorations
                if job.children.as_ref().is_some_and(|c| !c.is_empty()) {
                    if job.collapsed {
                        draw_collapsed_compound(ctx, node, job, theme);
                    } else {
                        draw_expanded_compound(ctx, node, job, theme);
                    }
                }
            }

            // Draw ports on all nodes (even custom-rendered ones)
            if !job.ports.is_empty() {
                draw_ports(ctx, node, job, theme);
            }
        }
    }

    // Draw rubber-band selection rectangle
    if let Some((x1, y1, x2, y2)) = callbacks.selection_rect {
        let rx = x1.min(x2);
        let ry = y1.min(y2);
        let rw = (x2 - x1).abs();
        let rh = (y2 - y1).abs();

        // Semi-transparent fill
        ctx.set_fill_style_str("rgba(59, 130, 246, 0.08)");
        ctx.fill_rect(rx, ry, rw, rh);

        // Dashed border
        ctx.set_stroke_style_str("rgba(59, 130, 246, 0.5)");
        ctx.set_line_width(1.0);
        ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from_f64(4.0),
            &JsValue::from_f64(3.0),
        )).ok();
        ctx.stroke_rect(rx, ry, rw, rh);
        ctx.set_line_dash(&js_sys::Array::new()).ok();
    }

    // Draw port connection preview line
    if let Some(drag) = port_drag {
        ctx.begin_path();
        ctx.set_stroke_style_str(&drag.color);
        ctx.set_line_width(2.0);
        ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from_f64(6.0),
            &JsValue::from_f64(4.0),
        ))?;
        let mid_x = (drag.from_x + drag.to_x) / 2.0;
        ctx.move_to(drag.from_x, drag.from_y);
        ctx.bezier_curve_to(mid_x, drag.from_y, mid_x, drag.to_y, drag.to_x, drag.to_y);
        ctx.stroke();
        ctx.set_line_dash(&js_sys::Array::new())?;
    }

    ctx.restore();

    // Draw minimap overlay (drawn in screen space, after restoring transform)
    if theme.minimap {
        draw_minimap(
            ctx, layout, &job_map, dpr, tw, th, zoom, pan_x, pan_y, theme,
        );
    }

    Ok(())
}

fn draw_header(ctx: &CanvasRenderingContext2d, workflow: &Workflow, theme: &ResolvedTheme) {
    let tl = &theme.layout;
    let fonts = &theme.fonts;
    let colors = &theme.colors;

    let x = tl.padding;
    let y = tl.padding + 10.0;

    ctx.set_font(&format!("bold {}px {}", fonts.size_header, fonts.family));
    ctx.set_fill_style_str(&colors.header_text);
    ctx.fill_text(&workflow.name, x, y).ok();

    ctx.set_font(&format!("{}px {}", fonts.size_header, fonts.family));
    ctx.set_fill_style_str(&colors.header_trigger);
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
    theme: &ResolvedTheme,
    style_override: Option<&EdgeStyle>,
) {
    let colors = &theme.colors;
    let tl = &theme.layout;
    let is_vertical = theme.direction == LayoutDirection::TopToBottom;

    let (x1, y1, x2, y2) = if is_vertical {
        (
            from.x + from.width / 2.0,
            from.y + from.height,
            to.x + to.width / 2.0,
            to.y,
        )
    } else {
        (
            from.x + from.width,
            from.y + from.height / 2.0,
            to.x,
            to.y + to.height / 2.0,
        )
    };

    // Resolve colors and width with overrides
    let (edge_color, junction_color, line_width) = if highlighted {
        (colors.highlight.as_str(), colors.highlight.as_str(), 2.5)
    } else {
        let ec = style_override
            .and_then(|s| s.color.as_deref())
            .unwrap_or(colors.edge.as_str());
        let lw = style_override.and_then(|s| s.width).unwrap_or(2.0);
        (ec, colors.junction.as_str(), lw)
    };

    ctx.begin_path();
    ctx.set_stroke_style_str(edge_color);
    ctx.set_line_width(line_width);

    // Apply dash pattern if specified
    if let Some(dash) = style_override.and_then(|s| s.dash.as_ref())
        && !dash.is_empty()
    {
        let arr = js_sys::Array::new();
        for &d in dash {
            arr.push(&JsValue::from_f64(d));
        }
        ctx.set_line_dash(&arr).ok();
    }

    ctx.move_to(x1, y1);
    if is_vertical {
        let mid_y = (y1 + y2) / 2.0;
        ctx.bezier_curve_to(x1, mid_y, x2, mid_y, x2, y2);
    } else {
        let mid_x = (x1 + x2) / 2.0;
        ctx.bezier_curve_to(mid_x, y1, mid_x, y2, x2, y2);
    }
    ctx.stroke();

    // Reset dash pattern
    if style_override.and_then(|s| s.dash.as_ref()).is_some() {
        ctx.set_line_dash(&js_sys::Array::new()).ok();
    }

    // Junction dot at source
    ctx.begin_path();
    ctx.set_fill_style_str(junction_color);
    ctx.arc(x1, y1, tl.junction_dot_radius, 0.0, 2.0 * PI).ok();
    ctx.fill();

    // Junction dot at target
    ctx.begin_path();
    ctx.arc(x2, y2, tl.junction_dot_radius, 0.0, 2.0 * PI).ok();
    ctx.fill();
}

fn draw_node(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    animation_time: f64,
    now_ms: f64,
    is_selected: bool,
    theme: &ResolvedTheme,
) {
    let colors = &theme.colors;
    let fonts = &theme.fonts;
    let tl = &theme.layout;

    // Node background
    draw_rounded_rect(ctx, node.x, node.y, node.width, node.height, tl.node_radius);
    ctx.set_fill_style_str(&colors.node_bg);
    ctx.fill();

    if is_selected {
        ctx.set_stroke_style_str(&colors.selected);
        ctx.set_line_width(2.0);
    } else {
        ctx.set_stroke_style_str(&colors.node_border);
        ctx.set_line_width(1.0);
    }
    ctx.stroke();

    // Status icon (top area)
    let icon_x = node.x + tl.status_icon_margin + tl.status_icon_radius;
    let icon_y = node.y + 14.0;
    draw_status_icon(ctx, icon_x, icon_y, &job.status, animation_time, theme);

    // Job name (top area, after icon)
    let text_x = icon_x + tl.status_icon_radius + 8.0;
    let text_y = node.y + 18.0;
    ctx.set_font(&format!("600 {}px {}", fonts.size_name, fonts.family));
    ctx.set_fill_style_str(&colors.text);
    ctx.fill_text(&job.name, text_x, text_y).ok();

    // Duration / live timer (right-aligned) — uses i18n labels
    let duration_str = match job.status {
        JobStatus::Running => {
            if let Some(started) = job.started_at {
                let elapsed_secs = ((now_ms - started) / 1000.0).max(0.0) as u64;
                Some(theme.labels.format_duration(elapsed_secs))
            } else {
                None
            }
        }
        JobStatus::Success | JobStatus::Failure => {
            job.duration_secs.map(|s| theme.labels.format_duration(s))
        }
        _ => None,
    };

    if let Some(dur_text) = duration_str {
        ctx.set_font(&format!("{}px {}", fonts.size_duration, fonts.family));
        let color = if job.status == JobStatus::Running {
            &colors.running
        } else {
            &colors.text_secondary
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

// ─── Production Node Rendering ────────────────────────────────────────────────

const HEADER_HEIGHT: f64 = 28.0;
const FIELD_ROW_HEIGHT: f64 = 22.0;
const _NODE_PADDING_BOTTOM: f64 = 8.0;
const PRODUCTION_RADIUS: f64 = 10.0;

#[allow(clippy::too_many_arguments)]
fn draw_production_node(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    def: &NodeDefinition,
    _animation_time: f64,
    _now_ms: f64,
    is_selected: bool,
    theme: &ResolvedTheme,
) {
    let colors = &theme.colors;
    let fonts = &theme.fonts;
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;

    // Node shadow
    ctx.save();
    ctx.set_shadow_color("rgba(0,0,0,0.25)");
    ctx.set_shadow_blur(8.0);
    ctx.set_shadow_offset_x(2.0);
    ctx.set_shadow_offset_y(2.0);

    // Node background
    draw_rounded_rect(ctx, x, y, w, h, PRODUCTION_RADIUS);
    ctx.set_fill_style_str(&colors.node_bg);
    ctx.fill();
    ctx.restore(); // restore shadow state

    // Selection or normal border
    draw_rounded_rect(ctx, x, y, w, h, PRODUCTION_RADIUS);
    if is_selected {
        ctx.set_stroke_style_str(&colors.selected);
        ctx.set_line_width(2.0);
    } else {
        ctx.set_stroke_style_str(&colors.node_border);
        ctx.set_line_width(1.0);
    }
    ctx.stroke();

    // ── Header bar ──
    let header_color = if !def.header_color.is_empty() {
        &def.header_color
    } else {
        // Fallback: check job.metadata for override
        job.metadata.get("header_color")
            .and_then(|v| v.as_str())
            .unwrap_or("#4b5563")
    };

    // Clip header to top rounded corners
    ctx.save();
    draw_rounded_rect_top(ctx, x, y, w, HEADER_HEIGHT, PRODUCTION_RADIUS);
    ctx.clip();
    ctx.set_fill_style_str(header_color);
    ctx.fill_rect(x, y, w, HEADER_HEIGHT);
    ctx.restore();

    // Header icon
    let icon = if !def.icon.is_empty() {
        &def.icon
    } else {
        job.metadata.get("icon")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    let mut text_x = x + 10.0;
    if !icon.is_empty() {
        ctx.set_font(&format!("13px {}", fonts.family));
        ctx.set_fill_style_str("#ffffff");
        ctx.fill_text(icon, text_x, y + 19.0).ok();
        text_x += 20.0;
    }

    // Header label
    ctx.set_font(&format!("bold 11px {}", fonts.family));
    ctx.set_fill_style_str("#ffffff");
    let label = &def.label;
    let max_label_w = w - (text_x - x) - 24.0; // leave room for status dot
    let measured = ctx.measure_text(label).map(|m| m.width()).unwrap_or(60.0);
    if measured > max_label_w {
        let chars = ((max_label_w / measured) * label.len() as f64) as usize;
        let truncated = format!("{}...", &label[..chars.saturating_sub(2)]);
        ctx.fill_text(&truncated, text_x, y + 19.0).ok();
    } else {
        ctx.fill_text(label, text_x, y + 19.0).ok();
    }

    // Status dot (top-right of header)
    let status_color = match job.status {
        JobStatus::Running => &colors.running,
        JobStatus::Success => &colors.success,
        JobStatus::Failure => &colors.failure,
        JobStatus::Queued => &colors.queued,
        JobStatus::Skipped => &colors.skipped,
        JobStatus::Cancelled => &colors.cancelled,
    };
    ctx.begin_path();
    ctx.arc(x + w - 12.0, y + HEADER_HEIGHT / 2.0, 4.0, 0.0, 2.0 * PI).ok();
    ctx.set_fill_style_str(status_color);
    ctx.fill();

    // ── Node name ──
    let name_y = y + HEADER_HEIGHT + 18.0;
    ctx.set_font(&format!("600 {}px {}", fonts.size_name, fonts.family));
    ctx.set_fill_style_str(&colors.text);
    let max_chars = ((w - 20.0) / 7.5) as usize;
    let display_name = if job.name.len() > max_chars {
        format!("{}...", &job.name[..max_chars.saturating_sub(3)])
    } else {
        job.name.clone()
    };
    ctx.fill_text(&display_name, x + 10.0, name_y).ok();

    // ── Inline fields ──
    let mut field_y = name_y + 6.0;
    for field_def in &def.fields {
        field_y += FIELD_ROW_HEIGHT;
        let value = job.metadata.get(&field_def.key)
            .map(|v| {
                v.as_str().map(String::from).unwrap_or_else(|| v.to_string())
            })
            .or_else(|| field_def.default_value.as_ref().map(|v| {
                v.as_str().map(String::from).unwrap_or_else(|| v.to_string())
            }));

        // Field label (left)
        ctx.set_font(&format!("10px {}", fonts.family));
        ctx.set_fill_style_str(&colors.text_secondary);
        ctx.fill_text(&field_def.label, x + 10.0, field_y).ok();

        // Field value (right-aligned)
        if let Some(val) = &value {
            ctx.set_fill_style_str(&colors.text);
            let max_val_w = w * 0.5;
            let val_measured = ctx.measure_text(val).map(|m| m.width()).unwrap_or(40.0);
            let display_val = if val_measured > max_val_w {
                let chars = ((max_val_w / val_measured) * val.len() as f64) as usize;
                format!("{}...", &val[..chars.saturating_sub(3)])
            } else {
                val.clone()
            };
            let val_w = ctx.measure_text(&display_val).map(|m| m.width()).unwrap_or(40.0);
            ctx.fill_text(&display_val, x + w - val_w - 10.0, field_y).ok();

            // Dropdown chevron for select fields
            if field_def.field_type == workflow_graph_shared::FieldType::Select {
                ctx.set_fill_style_str(&colors.text_secondary);
                ctx.fill_text("▾", x + w - 10.0, field_y).ok();
            }
        }
    }
}

// ─── Compound Node Rendering ──────────────────────────────────────────────────

/// Draw a collapsed compound node: stacked cards visual with child count badge.
fn draw_collapsed_compound(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    theme: &ResolvedTheme,
) {
    let colors = &theme.colors;
    let fonts = &theme.fonts;
    let child_count = job.children.as_ref().map(|c| c.len()).unwrap_or(0);
    if child_count == 0 {
        return;
    }

    // Draw stacked card effect (two offset rectangles behind the main node)
    let offsets = [6.0, 3.0];
    for offset in &offsets {
        ctx.save();
        ctx.set_global_alpha(0.3);
        draw_rounded_rect(ctx, node.x + offset, node.y - offset, node.width, node.height, 8.0);
        ctx.set_fill_style_str(&colors.node_bg);
        ctx.fill();
        ctx.set_stroke_style_str(&colors.node_border);
        ctx.set_line_width(1.0);
        ctx.stroke();
        ctx.restore();
    }

    // Child count badge (top-right)
    let badge_text = format!("{child_count}");
    let badge_w = 24.0_f64.max(badge_text.len() as f64 * 8.0 + 12.0);
    let badge_x = node.x + node.width - badge_w - 6.0;
    let badge_y = node.y + node.height - 20.0;

    draw_rounded_rect(ctx, badge_x, badge_y, badge_w, 16.0, 8.0);
    ctx.set_fill_style_str("#3b82f6");
    ctx.fill();

    ctx.set_font(&format!("bold 9px {}", fonts.family));
    ctx.set_fill_style_str("#ffffff");
    let text_w = ctx.measure_text(&badge_text).map(|m| m.width()).unwrap_or(8.0);
    ctx.fill_text(&badge_text, badge_x + (badge_w - text_w) / 2.0, badge_y + 12.0).ok();

    // Expand chevron
    ctx.set_fill_style_str(&colors.text_secondary);
    ctx.set_font(&format!("11px {}", fonts.family));
    ctx.fill_text("▶", node.x + 8.0, node.y + node.height - 8.0).ok();
}

/// Draw an expanded compound node: dashed border around group area with label.
fn draw_expanded_compound(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    theme: &ResolvedTheme,
) {
    let colors = &theme.colors;
    let fonts = &theme.fonts;

    // Dashed border around the group area
    ctx.save();
    draw_rounded_rect(ctx, node.x - 8.0, node.y - 8.0, node.width + 16.0, node.height + 16.0, 12.0);
    ctx.set_stroke_style_str(&colors.text_secondary);
    ctx.set_line_width(1.5);
    ctx.set_line_dash(&js_sys::Array::of2(
        &JsValue::from_f64(6.0),
        &JsValue::from_f64(4.0),
    )).ok();
    ctx.stroke();
    ctx.set_line_dash(&js_sys::Array::new()).ok();

    // Group label (top-left, above the dashed border)
    ctx.set_font(&format!("bold 10px {}", fonts.family));
    ctx.set_fill_style_str(&colors.text_secondary);
    ctx.fill_text(&job.name, node.x, node.y - 14.0).ok();

    // Collapse chevron
    ctx.set_fill_style_str(&colors.text_secondary);
    ctx.set_font(&format!("11px {}", fonts.family));
    let name_w = ctx.measure_text(&job.name).map(|m| m.width()).unwrap_or(40.0);
    ctx.fill_text("▼", node.x + name_w + 6.0, node.y - 14.0).ok();

    ctx.restore();
}

/// Draw a rounded rect with only the top corners rounded (for header clipping).
fn draw_rounded_rect_top(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64) {
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    ctx.arc_to(x + w, y, x + w, y + r, r).ok();
    ctx.line_to(x + w, y + h);
    ctx.line_to(x, y + h);
    ctx.line_to(x, y + r);
    ctx.arc_to(x, y, x + r, y, r).ok();
    ctx.close_path();
}

// ─── Minimap ─────────────────────────────────────────────────────────────────

const MINIMAP_WIDTH: f64 = 160.0;
const MINIMAP_HEIGHT: f64 = 100.0;
const MINIMAP_MARGIN: f64 = 12.0;

#[allow(clippy::too_many_arguments)]
fn draw_minimap(
    ctx: &CanvasRenderingContext2d,
    layout: &GraphLayout,
    job_map: &HashMap<&str, &workflow_graph_shared::Job>,
    dpr: f64,
    canvas_w: f64,
    canvas_h: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    theme: &ResolvedTheme,
) {
    if layout.nodes.is_empty() {
        return;
    }

    let colors = &theme.colors;

    // Minimap position (bottom-right corner, in screen coords)
    ctx.save();
    ctx.scale(dpr, dpr).ok();
    let mx = canvas_w - MINIMAP_WIDTH - MINIMAP_MARGIN;
    let my = canvas_h - MINIMAP_HEIGHT - MINIMAP_MARGIN;

    // Background
    ctx.set_global_alpha(0.85);
    draw_rounded_rect(ctx, mx, my, MINIMAP_WIDTH, MINIMAP_HEIGHT, 6.0);
    ctx.set_fill_style_str(&colors.graph_bg);
    ctx.fill();
    ctx.set_stroke_style_str(&colors.node_border);
    ctx.set_line_width(1.0);
    ctx.stroke();
    ctx.set_global_alpha(1.0);

    // Compute scale: fit the entire graph into the minimap
    let gw = layout.total_width.max(1.0);
    let gh = layout.total_height.max(1.0);
    let pad = 4.0;
    let scale_x = (MINIMAP_WIDTH - 2.0 * pad) / gw;
    let scale_y = (MINIMAP_HEIGHT - 2.0 * pad) / gh;
    let scale = scale_x.min(scale_y);

    let ox = mx + pad + (MINIMAP_WIDTH - 2.0 * pad - gw * scale) / 2.0;
    let oy = my + pad + (MINIMAP_HEIGHT - 2.0 * pad - gh * scale) / 2.0;

    // Draw nodes as small colored rectangles
    for node in &layout.nodes {
        let nx = ox + node.x * scale;
        let ny = oy + node.y * scale;
        let nw = node.width * scale;
        let nh = node.height * scale;

        let fill = if let Some(job) = job_map.get(node.job_id.as_str()) {
            match job.status {
                JobStatus::Success => &colors.success,
                JobStatus::Failure => &colors.failure,
                JobStatus::Running => &colors.running,
                _ => &colors.node_border,
            }
        } else {
            &colors.node_border
        };

        ctx.set_fill_style_str(fill);
        ctx.fill_rect(nx, ny, nw, nh);
    }

    // Draw viewport indicator
    let vx = ox + (-pan_x / zoom) * scale;
    let vy = oy + (-pan_y / zoom) * scale;
    let vw = (canvas_w / zoom) * scale;
    let vh = (canvas_h / zoom) * scale;

    ctx.set_stroke_style_str(&colors.highlight);
    ctx.set_line_width(1.5);
    ctx.stroke_rect(
        vx.max(mx + pad),
        vy.max(my + pad),
        vw.min(MINIMAP_WIDTH - 2.0 * pad),
        vh.min(MINIMAP_HEIGHT - 2.0 * pad),
    );

    ctx.restore();
}

/// Draw an edge between specific port positions.
///
/// `points` is `(x1, y1, x2, y2)`.
#[allow(clippy::too_many_arguments)]
fn draw_port_edge(
    ctx: &CanvasRenderingContext2d,
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    highlighted: bool,
    theme: &ResolvedTheme,
    style_override: Option<&EdgeStyle>,
) {
    let colors = &theme.colors;
    let color = style_override
        .and_then(|s| s.color.as_deref())
        .unwrap_or(if highlighted { &colors.highlight } else { &colors.edge });
    let width = style_override.and_then(|s| s.width).unwrap_or(if highlighted { 2.5 } else { 1.5 });

    ctx.begin_path();
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(width);

    if let Some(dash) = style_override.and_then(|s| s.dash.as_ref()) {
        let arr = js_sys::Array::new();
        for &d in dash {
            arr.push(&JsValue::from_f64(d));
        }
        ctx.set_line_dash(&arr).ok();
    }

    let mid_x = (x1 + x2) / 2.0;
    ctx.move_to(x1, y1);
    ctx.bezier_curve_to(mid_x, y1, mid_x, y2, x2, y2);
    ctx.stroke();

    ctx.set_line_dash(&js_sys::Array::new()).ok();
}

// ─── Ports ───────────────────────────────────────────────────────────────────

const PORT_RADIUS: f64 = 5.0;
const PORT_LABEL_OFFSET: f64 = 14.0;
const PORT_FONT_SIZE: f64 = 10.0;

/// Default colors for port types.
fn port_type_color(port_type: &str) -> &'static str {
    match port_type {
        "text" | "message" => "#3b82f6",    // blue
        "json" | "data" => "#8b5cf6",       // violet
        "tool_call" => "#f97316",           // orange
        "event" | "trigger" => "#22c55e",   // green
        "config" => "#6b7280",              // gray
        _ => "#9ca3af",                     // default gray
    }
}

fn draw_ports(
    ctx: &CanvasRenderingContext2d,
    node: &NodeLayout,
    job: &workflow_graph_shared::Job,
    theme: &ResolvedTheme,
) {
    let input_ports: Vec<_> = job
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input)
        .collect();
    let output_ports: Vec<_> = job
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Output)
        .collect();

    let fonts = &theme.fonts;

    // Draw input ports (left edge)
    for (i, port) in input_ports.iter().enumerate() {
        let px = node.x;
        let py = node.y + port_y_offset_render(i, input_ports.len(), node.height);
        let color = port.color.as_deref().unwrap_or_else(|| port_type_color(&port.port_type));

        // Port dot
        ctx.begin_path();
        ctx.arc(px, py, PORT_RADIUS, 0.0, 2.0 * PI).ok();
        ctx.set_fill_style_str(color);
        ctx.fill();

        // Port border
        ctx.set_stroke_style_str("#1f2937");
        ctx.set_line_width(1.5);
        ctx.stroke();

        // Port label (inside node, right of dot)
        ctx.set_font(&format!("{}px {}", PORT_FONT_SIZE, fonts.family));
        ctx.set_fill_style_str(color);
        ctx.fill_text(&port.label, px + PORT_LABEL_OFFSET, py + 3.5).ok();
    }

    // Draw output ports (right edge)
    for (i, port) in output_ports.iter().enumerate() {
        let px = node.x + node.width;
        let py = node.y + port_y_offset_render(i, output_ports.len(), node.height);
        let color = port.color.as_deref().unwrap_or_else(|| port_type_color(&port.port_type));

        // Port dot
        ctx.begin_path();
        ctx.arc(px, py, PORT_RADIUS, 0.0, 2.0 * PI).ok();
        ctx.set_fill_style_str(color);
        ctx.fill();

        // Port border
        ctx.set_stroke_style_str("#1f2937");
        ctx.set_line_width(1.5);
        ctx.stroke();

        // Port label (inside node, left of dot)
        ctx.set_font(&format!("{}px {}", PORT_FONT_SIZE, fonts.family));
        ctx.set_fill_style_str(color);
        let label_width = ctx.measure_text(&port.label).map(|m| m.width()).unwrap_or(40.0);
        ctx.fill_text(&port.label, px - PORT_LABEL_OFFSET - label_width, py + 3.5).ok();
    }
}

fn port_y_offset_render(index: usize, total: usize, node_height: f64) -> f64 {
    if total == 0 {
        return node_height / 2.0;
    }
    // Start ports below the node name area (top 28px reserved for name/header)
    let top_margin = 28.0;
    let usable_height = node_height - top_margin;
    let spacing = usable_height / (total as f64 + 1.0);
    top_margin + spacing * (index as f64 + 1.0)
}

// ─── Icons ───────────────────────────────────────────────────────────────────

// GitHub Octicon SVG path data (16x16 viewBox)
const OCTICON_CHECK_CIRCLE_FILL: &str = "M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-.018-1.042.751.751 0 0 0-1.042-.018L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042l2 2a.75.75 0 0 0 1.06 0Z";
const OCTICON_X_CIRCLE_FILL: &str = "M2.343 13.657A8 8 0 1 1 13.658 2.343 8 8 0 0 1 2.343 13.657ZM6.03 4.97a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042L6.94 8 4.97 9.97a.749.749 0 0 0 .326 1.275.749.749 0 0 0 .734-.215L8 9.06l1.97 1.97a.749.749 0 0 0 1.275-.326.749.749 0 0 0-.215-.734L9.06 8l1.97-1.97a.749.749 0 0 0-.326-1.275.749.749 0 0 0-.734.215L8 6.94Z";
const OCTICON_SKIP_FILL: &str = "M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm11.333-2.167a.825.825 0 0 0-1.166-1.166l-5.5 5.5a.825.825 0 0 0 1.166 1.166Z";
const OCTICON_CIRCLE: &str =
    "M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13Z";

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
    ctx.translate(cx - 8.0 * scale, cy - 8.0 * scale).ok();
    ctx.scale(scale, scale).ok();

    let path = web_sys::Path2d::new_with_path_string(path_data).unwrap();
    ctx.set_fill_style_str(color);
    ctx.fill_with_path_2d(&path);

    ctx.restore();
}

fn draw_status_icon(
    ctx: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    status: &JobStatus,
    animation_time: f64,
    theme: &ResolvedTheme,
) {
    let r = theme.layout.status_icon_radius;
    let colors = &theme.colors;

    match status {
        JobStatus::Queued => {
            draw_octicon(ctx, x, y, r, OCTICON_CIRCLE, &colors.queued);
        }
        JobStatus::Running => {
            let track_r = r - 1.0;
            let line_w = 2.5;

            let running_rgb = hex_to_rgb(&colors.running).unwrap_or((191, 135, 0));

            ctx.begin_path();
            ctx.set_stroke_style_str(&format!(
                "rgba({},{},{},0.15)",
                running_rgb.0, running_rgb.1, running_rgb.2
            ));
            ctx.set_line_width(line_w);
            ctx.arc(x, y, track_r, 0.0, 2.0 * PI).ok();
            ctx.stroke();

            let total_sweep = PI * 1.2;
            let head_angle = animation_time * 4.0;
            let segments = 20;
            let seg_sweep = total_sweep / segments as f64;
            ctx.save();
            ctx.set_line_cap("butt");
            ctx.set_line_width(line_w);
            for i in 0..segments {
                let t = i as f64 / segments as f64;
                let alpha = t * t;
                let seg_start = head_angle - total_sweep + (i as f64) * seg_sweep;
                ctx.begin_path();
                let color = format!(
                    "rgba({},{},{},{:.2})",
                    running_rgb.0, running_rgb.1, running_rgb.2, alpha
                );
                ctx.set_stroke_style_str(&color);
                ctx.arc(x, y, track_r, seg_start, seg_start + seg_sweep + 0.02)
                    .ok();
                ctx.stroke();
            }

            ctx.begin_path();
            ctx.set_stroke_style_str(&colors.running);
            ctx.set_line_width(line_w);
            ctx.set_line_cap("round");
            ctx.arc(x, y, track_r, head_angle - seg_sweep, head_angle)
                .ok();
            ctx.stroke();

            ctx.restore();
        }
        JobStatus::Success => {
            draw_octicon(ctx, x, y, r, OCTICON_CHECK_CIRCLE_FILL, &colors.success);
        }
        JobStatus::Failure => {
            draw_octicon(ctx, x, y, r, OCTICON_X_CIRCLE_FILL, &colors.failure);
        }
        JobStatus::Skipped => {
            draw_octicon(ctx, x, y, r, OCTICON_SKIP_FILL, &colors.skipped);
        }
        JobStatus::Cancelled => {
            draw_octicon(ctx, x, y, r, OCTICON_SKIP_FILL, &colors.cancelled);
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

/// Parse a hex color string (#RRGGBB) into (r, g, b) components.
fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}
