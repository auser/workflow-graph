mod layout;
mod render;
pub mod theme;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement, KeyboardEvent, MouseEvent,
    ResizeObserver, ResizeObserverEntry, WheelEvent,
};

use layout::GraphLayout;
use theme::ResolvedTheme;
use workflow_graph_shared::{JobStatus, Workflow};

const CLICK_THRESHOLD: f64 = 5.0;
const MIN_ZOOM: f64 = 0.25;
const MAX_ZOOM: f64 = 4.0;
const ZOOM_SPEED: f64 = 0.001;

/// Persistent state for an interactive graph instance.
struct GraphState {
    workflow: Workflow,
    layout: GraphLayout,
    initial_layout: GraphLayout,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    dpr: f64,
    canvas_width: f64,
    canvas_height: f64,
    // Theme
    theme: ResolvedTheme,
    // Drag
    dragging: Option<usize>,
    drag_offset_x: f64,
    drag_offset_y: f64,
    // Pan & Zoom
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    panning: bool,
    pan_start_x: f64,
    pan_start_y: f64,
    pan_start_pan_x: f64,
    pan_start_pan_y: f64,
    // Hover & highlight
    hovered: Option<usize>,
    highlighted_edges: Vec<usize>,
    // Selection
    selected: HashSet<String>,
    // Animation
    animating: bool,
    // Callbacks
    on_node_click: Option<js_sys::Function>,
    on_node_hover: Option<js_sys::Function>,
    on_node_drag_end: Option<js_sys::Function>,
    on_canvas_click: Option<js_sys::Function>,
    on_selection_change: Option<js_sys::Function>,
    on_edge_click: Option<js_sys::Function>,
    on_render_node: Option<js_sys::Function>,
    // Click detection
    mouse_down_pos: Option<(f64, f64)>,
    // Resize
    auto_resize: bool,
    _resize_observer: Option<ResizeObserver>,
    // Accessibility
    live_region: Option<web_sys::HtmlElement>,
    last_announced: String,
}

impl GraphState {
    fn redraw_with_time(&self, animation_time: f64, now_ms: f64) {
        let tw = self.canvas_width;
        let th = self.canvas_height;
        self.canvas.set_width((tw * self.dpr) as u32);
        self.canvas.set_height((th * self.dpr) as u32);
        let html_el: &HtmlElement = self.canvas.unchecked_ref();
        html_el
            .style()
            .set_property("width", &format!("{tw}px"))
            .ok();
        html_el
            .style()
            .set_property("height", &format!("{th}px"))
            .ok();

        render::render_with_callbacks(
            &self.ctx,
            &self.workflow,
            &self.layout,
            self.dpr,
            &self.highlighted_edges,
            tw,
            th,
            animation_time,
            now_ms,
            self.zoom,
            self.pan_x,
            self.pan_y,
            &self.selected,
            &self.theme,
            &render::RenderCallbacks {
                on_render_node: self.on_render_node.as_ref(),
            },
        )
        .ok();
    }

    fn redraw(&self) {
        let now = js_sys::Date::now();
        self.redraw_with_time(now / 1000.0, now);
    }

    fn has_running_jobs(&self) -> bool {
        self.workflow
            .jobs
            .iter()
            .any(|j| j.status == JobStatus::Running)
    }

    fn hit_test(&self, x: f64, y: f64) -> Option<usize> {
        let gx = (x - self.pan_x) / self.zoom;
        let gy = (y - self.pan_y) / self.zoom;
        for (i, node) in self.layout.nodes.iter().enumerate() {
            if gx >= node.x
                && gx <= node.x + node.width
                && gy >= node.y
                && gy <= node.y + node.height
            {
                return Some(i);
            }
        }
        None
    }

    fn screen_to_graph(&self, x: f64, y: f64) -> (f64, f64) {
        ((x - self.pan_x) / self.zoom, (y - self.pan_y) / self.zoom)
    }

    fn compute_highlighted_path(&mut self, node_idx: Option<usize>) {
        self.highlighted_edges.clear();
        let Some(idx) = node_idx else { return };
        let job_id = &self.layout.nodes[idx].job_id;

        let mut ancestor_ids: Vec<String> = vec![job_id.clone()];
        let mut descendant_ids: Vec<String> = vec![job_id.clone()];

        let mut stack = vec![job_id.clone()];
        while let Some(current) = stack.pop() {
            if let Some(job) = self.workflow.jobs.iter().find(|j| j.id == current) {
                for dep in &job.depends_on {
                    if !ancestor_ids.contains(dep) {
                        ancestor_ids.push(dep.clone());
                        stack.push(dep.clone());
                    }
                }
            }
        }

        let mut stack = vec![job_id.clone()];
        while let Some(current) = stack.pop() {
            for job in &self.workflow.jobs {
                if job.depends_on.contains(&current) && !descendant_ids.contains(&job.id) {
                    descendant_ids.push(job.id.clone());
                    stack.push(job.id.clone());
                }
            }
        }

        let all_ids: Vec<&str> = ancestor_ids
            .iter()
            .chain(descendant_ids.iter())
            .map(|s| s.as_str())
            .collect();

        for (i, edge) in self.layout.edges.iter().enumerate() {
            if all_ids.contains(&edge.from_id.as_str()) && all_ids.contains(&edge.to_id.as_str()) {
                self.highlighted_edges.push(i);
            }
        }
    }

    fn fire_selection_change(&self) {
        if let Some(ref cb) = self.on_selection_change {
            let arr = js_sys::Array::new();
            for id in &self.selected {
                arr.push(&JsValue::from_str(id));
            }
            cb.call1(&JsValue::NULL, &arr).ok();
        }
    }

    /// Hit-test edges using distance-to-bezier approximation.
    fn edge_hit_test(&self, x: f64, y: f64) -> Option<usize> {
        use crate::theme::LayoutDirection;
        let gx = (x - self.pan_x) / self.zoom;
        let gy = (y - self.pan_y) / self.zoom;
        let threshold = 6.0;

        let node_map: HashMap<&str, &layout::NodeLayout> = self
            .layout
            .nodes
            .iter()
            .map(|n| (n.job_id.as_str(), n))
            .collect();

        let is_vertical = self.theme.direction == LayoutDirection::TopToBottom;

        for (i, edge) in self.layout.edges.iter().enumerate() {
            let (Some(from), Some(to)) = (
                node_map.get(edge.from_id.as_str()),
                node_map.get(edge.to_id.as_str()),
            ) else {
                continue;
            };

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

            // Sample the bezier at N points and check distance
            for t_int in 0..=20 {
                let t = t_int as f64 / 20.0;
                let (bx, by) = if is_vertical {
                    let mid_y = (y1 + y2) / 2.0;
                    bezier_point(x1, y1, x1, mid_y, x2, mid_y, x2, y2, t)
                } else {
                    let mid_x = (x1 + x2) / 2.0;
                    bezier_point(x1, y1, mid_x, y1, mid_x, y2, x2, y2, t)
                };
                let dx: f64 = gx - bx;
                let dy: f64 = gy - by;
                if (dx * dx + dy * dy).sqrt() < threshold {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Announce a message to screen readers via the ARIA live region.
    fn announce(&mut self, message: &str) {
        if message == self.last_announced {
            return;
        }
        self.last_announced = message.to_string();
        if let Some(ref el) = self.live_region {
            el.set_text_content(Some(message));
        }
    }

    /// Check for status changes and announce them.
    fn announce_status_changes(&mut self, old_statuses: &HashMap<String, JobStatus>) {
        let labels = &self.theme.labels;
        let mut announcements = Vec::new();
        for job in &self.workflow.jobs {
            if let Some(old) = old_statuses.get(&job.id) {
                if *old != job.status {
                    let status_label = match job.status {
                        JobStatus::Queued => &labels.queued,
                        JobStatus::Running => &labels.running,
                        JobStatus::Success => &labels.success,
                        JobStatus::Failure => &labels.failure,
                        JobStatus::Skipped => &labels.skipped,
                        JobStatus::Cancelled => &labels.cancelled,
                    };
                    announcements.push(format!("{}: {}", job.name, status_label));
                }
            }
        }
        if !announcements.is_empty() {
            self.announce(&announcements.join(". "));
        }
    }
}

type SharedState = Rc<RefCell<GraphState>>;

/// Stored event listener that can be cleaned up on destroy.
struct StoredListener {
    event: &'static str,
    /// The JS function reference for removeEventListener.
    js_fn: js_sys::Function,
    /// The Closure kept alive to prevent GC. Dropped on destroy.
    _closure: Box<dyn std::any::Any>,
}

/// Holds a graph instance's state and its event listener closures for cleanup.
struct GraphInstance {
    state: SharedState,
    /// Event listeners attached to the canvas — removed on destroy.
    listeners: Vec<StoredListener>,
}

thread_local! {
    static GRAPHS: RefCell<HashMap<String, GraphInstance>> = RefCell::new(HashMap::new());
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    Ok(())
}

/// Initialize an interactive workflow graph on a canvas.
///
/// # Arguments
/// - `canvas_id` — ID of the `<canvas>` element
/// - `workflow_json` — JSON string of the `Workflow` data
/// - `on_node_click` — optional callback: `(jobId: string) => void`
/// - `on_node_hover` — optional callback: `(jobId: string | null) => void`
/// - `on_canvas_click` — optional callback: `() => void`
/// - `on_selection_change` — optional callback: `(selectedIds: string[]) => void`
/// - `on_node_drag_end` — optional callback: `(jobId: string, x: number, y: number) => void`
/// - `theme_json` — optional JSON string of `ThemeConfig` for custom colors, fonts, layout, direction
#[wasm_bindgen]
pub fn render_workflow(
    canvas_id: &str,
    workflow_json: &str,
    on_node_click: Option<js_sys::Function>,
    on_node_hover: Option<js_sys::Function>,
    on_canvas_click: Option<js_sys::Function>,
    on_selection_change: Option<js_sys::Function>,
    on_node_drag_end: Option<js_sys::Function>,
    theme_json: Option<String>,
) -> Result<(), JsValue> {
    let workflow: Workflow = serde_json::from_str(workflow_json)
        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {e}")))?;

    let theme_config: Option<theme::ThemeConfig> = match theme_json {
        Some(ref json) if !json.is_empty() => serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Theme JSON parse error: {e}")))?,
        _ => None,
    };
    let resolved_theme = ResolvedTheme::from_config(theme_config);

    let graph_layout = layout::compute_layout(&workflow, &resolved_theme);

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no canvas element '{canvas_id}'")))?
        .dyn_into::<HtmlCanvasElement>()?;

    let dpr = window.device_pixel_ratio();
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    // Create ARIA live region for status announcements
    let live_region = create_live_region(&document, &canvas)?;

    let state = Rc::new(RefCell::new(GraphState {
        workflow,
        canvas_width: graph_layout.total_width,
        canvas_height: graph_layout.total_height,
        initial_layout: graph_layout.clone(),
        layout: graph_layout,
        canvas: canvas.clone(),
        ctx,
        dpr,
        theme: resolved_theme,
        dragging: None,
        drag_offset_x: 0.0,
        drag_offset_y: 0.0,
        zoom: 1.0,
        pan_x: 0.0,
        pan_y: 0.0,
        panning: false,
        pan_start_x: 0.0,
        pan_start_y: 0.0,
        pan_start_pan_x: 0.0,
        pan_start_pan_y: 0.0,
        hovered: None,
        highlighted_edges: vec![],
        selected: HashSet::new(),
        animating: false,
        on_node_click,
        on_node_hover,
        on_canvas_click,
        on_selection_change,
        on_node_drag_end,
        on_edge_click: None,
        on_render_node: None,
        mouse_down_pos: None,
        auto_resize: false,
        _resize_observer: None,
        live_region: Some(live_region),
        last_announced: String::new(),
    }));

    state.borrow().redraw();
    let listeners = attach_event_handlers(&canvas, &state)?;

    // Listen for DPR changes (e.g., window moved between displays)
    {
        let state = state.clone();
        let dpr = window.device_pixel_ratio();
        let query = format!("(resolution: {dpr}dppx)");
        if let Ok(mql) = window.match_media(&query) {
            if let Some(mql) = mql {
                let closure = Closure::<dyn FnMut()>::new(move || {
                    if let Some(win) = web_sys::window() {
                        let new_dpr = win.device_pixel_ratio();
                        if let Ok(mut s) = state.try_borrow_mut() {
                            if (s.dpr - new_dpr).abs() > 0.01 {
                                s.dpr = new_dpr;
                                s.redraw();
                            }
                        }
                    }
                });
                mql.add_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()))
                    .ok();
                closure.forget(); // Intentional: lives for the lifetime of the page
            }
        }
    }

    let id = canvas_id.to_string();
    let instance = GraphInstance { state: state.clone(), listeners };
    GRAPHS.with(|g| g.borrow_mut().insert(id.clone(), instance));
    maybe_start_animation(&id, &state);

    Ok(())
}

/// Update workflow data without resetting node positions, zoom, or selection.
#[wasm_bindgen]
pub fn update_workflow_data(canvas_id: &str, workflow_json: &str) -> Result<(), JsValue> {
    let new_workflow: Workflow = serde_json::from_str(workflow_json)
        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {e}")))?;

    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let state = &instance.state;
            let mut s = state.borrow_mut();
            // Snapshot old statuses for a11y announcements
            let old_statuses: HashMap<String, JobStatus> = s
                .workflow
                .jobs
                .iter()
                .map(|j| (j.id.clone(), j.status.clone()))
                .collect();

            for job in &new_workflow.jobs {
                if let Some(existing) = s.workflow.jobs.iter_mut().find(|j| j.id == job.id) {
                    existing.status = job.status.clone();
                    existing.duration_secs = job.duration_secs;
                    existing.started_at = job.started_at;
                    existing.output = job.output.clone();
                }
            }
            s.announce_status_changes(&old_statuses);
            if s.dragging.is_none() {
                s.redraw();
            }
            let has_running = s.has_running_jobs();
            let already_animating = s.animating;
            drop(s);
            if has_running && !already_animating {
                maybe_start_animation(canvas_id, state);
            }
            Ok(())
        } else {
            drop(graphs);
            render_workflow(canvas_id, workflow_json, None, None, None, None, None, None)
        }
    })
}

/// Update the theme at runtime without resetting state.
#[wasm_bindgen]
pub fn set_theme(canvas_id: &str, theme_json: &str) -> Result<(), JsValue> {
    let theme_config: theme::ThemeConfig = serde_json::from_str(theme_json)
        .map_err(|e| JsValue::from_str(&format!("Theme JSON parse error: {e}")))?;
    let resolved = ResolvedTheme::from_config(Some(theme_config));

    with_state(canvas_id, |s| {
        // Re-compute layout if direction or dimensions changed
        let needs_relayout = s.theme.direction != resolved.direction
            || s.theme.layout.node_width != resolved.layout.node_width
            || s.theme.layout.node_height != resolved.layout.node_height
            || s.theme.layout.h_gap != resolved.layout.h_gap
            || s.theme.layout.v_gap != resolved.layout.v_gap;

        s.theme = resolved;

        if needs_relayout {
            let new_layout = layout::compute_layout(&s.workflow, &s.theme);
            s.canvas_width = new_layout.total_width;
            s.canvas_height = new_layout.total_height;
            s.initial_layout = new_layout.clone();
            s.layout = new_layout;
        }

        s.redraw();
    });

    Ok(())
}

/// Enable or disable auto-resize via ResizeObserver on the canvas parent.
#[wasm_bindgen]
pub fn set_auto_resize(canvas_id: &str, enabled: bool) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let state = &instance.state;
            let mut s = state.borrow_mut();
            if enabled && !s.auto_resize {
                let parent = s
                    .canvas
                    .parent_element()
                    .ok_or_else(|| JsValue::from_str("canvas has no parent element"))?;

                let state_clone = state.clone();
                let closure = Closure::<dyn FnMut(js_sys::Array, ResizeObserver)>::new(
                    move |entries: js_sys::Array, _observer: ResizeObserver| {
                        let entry: ResizeObserverEntry = match entries.get(0).dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let rect = entry.content_rect();
                        let w = rect.width();
                        let h = rect.height();
                        if w > 0.0 && h > 0.0 {
                            if let Ok(mut s) = state_clone.try_borrow_mut() {
                                s.canvas_width = w;
                                s.canvas_height = h;
                                s.redraw();
                            }
                        }
                    },
                );

                let observer = ResizeObserver::new(closure.as_ref().unchecked_ref())?;
                observer.observe(&parent);
                closure.forget(); // ResizeObserver closure — cleaned up via observer.disconnect()
                s._resize_observer = Some(observer);
                s.auto_resize = true;
            } else if !enabled && s.auto_resize {
                if let Some(observer) = s._resize_observer.take() {
                    observer.disconnect();
                }
                s.auto_resize = false;
            }
            Ok(())
        } else {
            Ok(())
        }
    })
}

/// Return the dark theme preset as a JSON string consumers can pass to render_workflow.
#[wasm_bindgen]
pub fn get_dark_theme() -> String {
    let config = theme::ThemeConfig {
        colors: Some(theme::dark_theme_colors()),
        ..Default::default()
    };
    serde_json::to_string(&config).unwrap_or_default()
}

/// Return the high-contrast accessibility theme preset as a JSON string.
#[wasm_bindgen]
pub fn get_high_contrast_theme() -> String {
    let config = theme::ThemeConfig {
        colors: Some(theme::high_contrast_colors()),
        ..Default::default()
    };
    serde_json::to_string(&config).unwrap_or_default()
}

// ─── Programmatic Control API ────────────────────────────────────────────────

#[wasm_bindgen]
pub fn select_node(canvas_id: &str, job_id: &str) {
    with_state(canvas_id, |s| {
        s.selected.insert(job_id.to_string());
        s.fire_selection_change();
        s.redraw();
    });
}

#[wasm_bindgen]
pub fn deselect_all(canvas_id: &str) {
    with_state(canvas_id, |s| {
        s.selected.clear();
        s.fire_selection_change();
        s.redraw();
    });
}

#[wasm_bindgen]
pub fn reset_layout(canvas_id: &str) {
    with_state(canvas_id, |s| {
        s.layout = s.initial_layout.clone();
        s.zoom = 1.0;
        s.pan_x = 0.0;
        s.pan_y = 0.0;
        s.redraw();
    });
}

#[wasm_bindgen]
pub fn zoom_to_fit(canvas_id: &str) {
    with_state(canvas_id, |s| {
        s.zoom = 1.0;
        s.pan_x = 0.0;
        s.pan_y = 0.0;
        s.redraw();
    });
}

#[wasm_bindgen]
pub fn set_zoom(canvas_id: &str, level: f64) {
    with_state(canvas_id, |s| {
        s.zoom = level.clamp(MIN_ZOOM, MAX_ZOOM);
        s.redraw();
    });
}

#[wasm_bindgen]
pub fn get_node_positions(canvas_id: &str) -> JsValue {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let s = instance.state.borrow();
            let positions: HashMap<&str, (f64, f64)> = s
                .layout
                .nodes
                .iter()
                .map(|n| (n.job_id.as_str(), (n.x, n.y)))
                .collect();
            serde_wasm_bindgen::to_value(&positions).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    })
}

#[wasm_bindgen]
pub fn set_node_positions(canvas_id: &str, positions_json: &str) {
    let positions: HashMap<String, (f64, f64)> =
        serde_json::from_str(positions_json).unwrap_or_default();
    with_state(canvas_id, |s| {
        for node in &mut s.layout.nodes {
            if let Some(&(x, y)) = positions.get(&node.job_id) {
                node.x = x;
                node.y = y;
            }
        }
        s.redraw();
    });
}

/// Set an edge click callback: `(fromId: string, toId: string) => void`.
#[wasm_bindgen]
pub fn set_on_edge_click(canvas_id: &str, callback: js_sys::Function) {
    with_state(canvas_id, |s| {
        s.on_edge_click = Some(callback);
    });
}

/// Set a custom node render callback: `(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, job: object) => boolean`.
/// Return `true` to skip default node rendering, `false` to render the default on top.
#[wasm_bindgen]
pub fn set_on_render_node(canvas_id: &str, callback: js_sys::Function) {
    with_state(canvas_id, |s| {
        s.on_render_node = Some(callback);
    });
}

#[wasm_bindgen]
pub fn destroy(canvas_id: &str) {
    GRAPHS.with(|g| {
        if let Some(instance) = g.borrow_mut().remove(canvas_id) {
            let s = instance.state.borrow();
            // Disconnect resize observer
            if let Some(ref observer) = s._resize_observer {
                observer.disconnect();
            }
            // Remove ARIA live region
            if let Some(ref el) = s.live_region {
                el.remove();
            }
            // Remove event listeners
            let canvas = s.canvas.clone();
            drop(s);
            for listener in &instance.listeners {
                canvas
                    .remove_event_listener_with_callback(
                        listener.event,
                        &listener.js_fn,
                    )
                    .ok();
            }
            // instance.listeners dropped here — closures freed
        }
    });
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn with_state(canvas_id: &str, f: impl FnOnce(&mut GraphState)) {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            f(&mut instance.state.borrow_mut());
        }
    });
}

fn maybe_start_animation(canvas_id: &str, state: &SharedState) {
    let s = state.borrow();
    if !s.has_running_jobs() || s.animating {
        return;
    }
    drop(s);

    state.borrow_mut().animating = true;
    let state = state.clone();
    let _canvas_id = canvas_id.to_string();

    type AnimCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
    let callback: AnimCallback = Rc::new(RefCell::new(None));
    let callback_clone = callback.clone();

    *callback.borrow_mut() = Some(Closure::new(move |_timestamp: f64| {
        let should_continue = match state.try_borrow() {
            Ok(s) => {
                if !s.has_running_jobs() {
                    false
                } else {
                    let now = js_sys::Date::now();
                    s.redraw_with_time(now / 1000.0, now);
                    true
                }
            }
            Err(_) => true,
        };

        if should_continue {
            if let Some(window) = web_sys::window()
                && let Some(cb) = callback_clone.borrow().as_ref()
            {
                window
                    .request_animation_frame(cb.as_ref().unchecked_ref())
                    .ok();
            }
        } else {
            state.borrow_mut().animating = false;
            *callback_clone.borrow_mut() = None;
        }
    }));

    if let Some(window) = web_sys::window()
        && let Some(cb) = callback.borrow().as_ref()
    {
        window
            .request_animation_frame(cb.as_ref().unchecked_ref())
            .ok();
    }
}

fn attach_event_handlers(
    canvas: &HtmlCanvasElement,
    state: &SharedState,
) -> Result<Vec<StoredListener>, JsValue> {
    let mut listeners: Vec<StoredListener> = Vec::new();

    macro_rules! add_listener {
        ($event:expr, $closure:expr) => {{
            let closure = $closure;
            let js_fn: js_sys::Function =
                closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
            canvas.add_event_listener_with_callback($event, &js_fn)?;
            listeners.push(StoredListener {
                event: $event,
                js_fn,
                _closure: Box::new(closure),
            });
        }};
    }
    // mousedown
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let (mx, my) = mouse_pos(&event, &state);
            let mut s = state.borrow_mut();
            s.mouse_down_pos = Some((mx, my));

            if let Some(idx) = s.hit_test(mx, my) {
                let (gx, gy) = s.screen_to_graph(mx, my);
                s.dragging = Some(idx);
                s.drag_offset_x = gx - s.layout.nodes[idx].x;
                s.drag_offset_y = gy - s.layout.nodes[idx].y;
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "grabbing").ok();
            } else {
                s.panning = true;
                s.pan_start_x = mx;
                s.pan_start_y = my;
                s.pan_start_pan_x = s.pan_x;
                s.pan_start_pan_y = s.pan_y;
            }
        });
        add_listener!("mousedown", closure);
    }

    // mousemove
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let (mx, my) = mouse_pos(&event, &state);
            let mut s = state.borrow_mut();

            if let Some(idx) = s.dragging {
                let (gx, gy) = s.screen_to_graph(mx, my);
                let node_w = s.layout.nodes[idx].width;
                let node_h = s.layout.nodes[idx].height;
                let new_x = (gx - s.drag_offset_x).clamp(0.0, s.canvas_width - node_w);
                let new_y = (gy - s.drag_offset_y).clamp(0.0, s.canvas_height - node_h);
                s.layout.nodes[idx].x = new_x;
                s.layout.nodes[idx].y = new_y;
                s.redraw();
            } else if s.panning {
                let dx = mx - s.pan_start_x;
                let dy = my - s.pan_start_y;
                s.pan_x = s.pan_start_pan_x + dx;
                s.pan_y = s.pan_start_pan_y + dy;
                s.redraw();
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "move").ok();
            } else {
                let new_hover = s.hit_test(mx, my);
                let changed = new_hover != s.hovered;

                if changed {
                    s.hovered = new_hover;
                    s.compute_highlighted_path(new_hover);
                    s.redraw();

                    if let Some(ref cb) = s.on_node_hover {
                        let val = match new_hover {
                            Some(idx) => JsValue::from_str(&s.layout.nodes[idx].job_id),
                            None => JsValue::NULL,
                        };
                        cb.call1(&JsValue::NULL, &val).ok();
                    }
                }

                let cursor = if new_hover.is_some() {
                    "grab"
                } else {
                    "default"
                };
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", cursor).ok();
            }
        });
        add_listener!("mousemove", closure);
    }

    // mouseup
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let (mx, my) = mouse_pos(&event, &state);
            let mut s = state.borrow_mut();

            let is_click = s
                .mouse_down_pos
                .map(|(dx, dy)| ((mx - dx).powi(2) + (my - dy).powi(2)).sqrt() < CLICK_THRESHOLD)
                .unwrap_or(false);

            if is_click {
                if let Some(idx) = s.hit_test(mx, my) {
                    let job_id = s.layout.nodes[idx].job_id.clone();

                    if event.shift_key() {
                        if s.selected.contains(&job_id) {
                            s.selected.remove(&job_id);
                        } else {
                            s.selected.insert(job_id.clone());
                        }
                    } else {
                        s.selected.clear();
                        s.selected.insert(job_id.clone());
                    }
                    s.fire_selection_change();

                    if let Some(ref cb) = s.on_node_click {
                        cb.call1(&JsValue::NULL, &JsValue::from_str(&job_id)).ok();
                    }
                    s.redraw();
                } else {
                    // Check edge click before firing canvas click
                    let mut edge_clicked = false;
                    if let Some(ref cb) = s.on_edge_click {
                        if let Some(edge_idx) = s.edge_hit_test(mx, my) {
                            let edge = &s.layout.edges[edge_idx];
                            cb.call2(
                                &JsValue::NULL,
                                &JsValue::from_str(&edge.from_id),
                                &JsValue::from_str(&edge.to_id),
                            )
                            .ok();
                            edge_clicked = true;
                        }
                    }

                    if !edge_clicked {
                        if !s.selected.is_empty() {
                            s.selected.clear();
                            s.fire_selection_change();
                            s.redraw();
                        }
                        if let Some(ref cb) = s.on_canvas_click {
                            cb.call0(&JsValue::NULL).ok();
                        }
                    }
                }
            }

            if let Some(idx) = s.dragging
                && !is_click
                && let Some(ref cb) = s.on_node_drag_end
            {
                let node = &s.layout.nodes[idx];
                let _ = cb.call3(
                    &JsValue::NULL,
                    &JsValue::from_str(&node.job_id),
                    &JsValue::from_f64(node.x),
                    &JsValue::from_f64(node.y),
                );
            }

            s.mouse_down_pos = None;
            s.dragging = None;
            s.panning = false;
            let html: &HtmlElement = s.canvas.unchecked_ref();
            html.style().set_property("cursor", "default").ok();
        });
        add_listener!("mouseup", closure);
    }

    // mouseleave
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(MouseEvent)>::new(move |_event: MouseEvent| {
            let mut s = state.borrow_mut();
            s.dragging = None;
            s.panning = false;
            s.mouse_down_pos = None;
            let had_hover = s.hovered.is_some();
            s.hovered = None;
            s.highlighted_edges.clear();
            let html: &HtmlElement = s.canvas.unchecked_ref();
            html.style().set_property("cursor", "default").ok();
            if had_hover {
                if let Some(ref cb) = s.on_node_hover {
                    cb.call1(&JsValue::NULL, &JsValue::NULL).ok();
                }
                s.redraw();
            }
        });
        add_listener!("mouseleave", closure);
    }

    // wheel (zoom)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();
            let mut s = state.borrow_mut();

            let (mx, my) = {
                let rect = s.canvas.get_bounding_client_rect();
                (
                    event.client_x() as f64 - rect.left(),
                    event.client_y() as f64 - rect.top(),
                )
            };

            let old_zoom = s.zoom;
            let delta = -event.delta_y() * ZOOM_SPEED;
            s.zoom = (s.zoom * (1.0 + delta)).clamp(MIN_ZOOM, MAX_ZOOM);

            let scale_change = s.zoom / old_zoom;
            s.pan_x = mx - (mx - s.pan_x) * scale_change;
            s.pan_y = my - (my - s.pan_y) * scale_change;

            s.redraw();
        });
        add_listener!("wheel", closure);
    }

    // keydown (Tab to cycle nodes, Enter/Space to select, Escape to deselect)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            let mut s = state.borrow_mut();
            let key = event.key();

            match key.as_str() {
                "Tab" => {
                    event.prevent_default();
                    let node_count = s.layout.nodes.len();
                    if node_count == 0 {
                        return;
                    }

                    let current_idx = if s.selected.len() == 1 {
                        let selected_id = s.selected.iter().next().unwrap();
                        s.layout.nodes.iter().position(|n| n.job_id == *selected_id)
                    } else {
                        None
                    };

                    let next_idx = if event.shift_key() {
                        match current_idx {
                            Some(i) if i > 0 => i - 1,
                            _ => node_count - 1,
                        }
                    } else {
                        match current_idx {
                            Some(i) => (i + 1) % node_count,
                            None => 0,
                        }
                    };

                    let job_id = s.layout.nodes[next_idx].job_id.clone();
                    s.selected.clear();
                    s.selected.insert(job_id);
                    s.fire_selection_change();
                    s.redraw();
                }
                "Enter" | " " => {
                    event.prevent_default();
                    if s.selected.len() == 1 {
                        let job_id = s.selected.iter().next().unwrap().clone();
                        if let Some(ref cb) = s.on_node_click {
                            cb.call1(&JsValue::NULL, &JsValue::from_str(&job_id)).ok();
                        }
                    }
                }
                "Escape" => {
                    if !s.selected.is_empty() {
                        s.selected.clear();
                        s.fire_selection_change();
                        s.redraw();
                    }
                }
                _ => {}
            }
        });
        add_listener!("keydown", closure);
    }

    // Prevent browser default touch behaviors (scroll/zoom) on the canvas
    {
        let html: &HtmlElement = canvas.unchecked_ref();
        html.style().set_property("touch-action", "none").ok();
    }

    // touchstart — mirrors mousedown logic
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(web_sys::TouchEvent)>::new(
            move |event: web_sys::TouchEvent| {
                event.prevent_default();
                let Some(touch) = event.touches().get(0) else {
                    return;
                };
                let (mx, my) = touch_pos(&touch, &state);
                let mut s = state.borrow_mut();
                s.mouse_down_pos = Some((mx, my));

                if let Some(idx) = s.hit_test(mx, my) {
                    let (gx, gy) = s.screen_to_graph(mx, my);
                    s.dragging = Some(idx);
                    s.drag_offset_x = gx - s.layout.nodes[idx].x;
                    s.drag_offset_y = gy - s.layout.nodes[idx].y;
                } else {
                    s.panning = true;
                    s.pan_start_x = mx;
                    s.pan_start_y = my;
                    s.pan_start_pan_x = s.pan_x;
                    s.pan_start_pan_y = s.pan_y;
                }
            },
        );
        add_listener!("touchstart", closure);
    }

    // touchmove — mirrors mousemove logic
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(web_sys::TouchEvent)>::new(
            move |event: web_sys::TouchEvent| {
                event.prevent_default();
                let Some(touch) = event.touches().get(0) else {
                    return;
                };
                let (mx, my) = touch_pos(&touch, &state);
                let mut s = state.borrow_mut();

                if let Some(idx) = s.dragging {
                    let (gx, gy) = s.screen_to_graph(mx, my);
                    let node_w = s.layout.nodes[idx].width;
                    let node_h = s.layout.nodes[idx].height;
                    let new_x = (gx - s.drag_offset_x).clamp(0.0, s.canvas_width - node_w);
                    let new_y = (gy - s.drag_offset_y).clamp(0.0, s.canvas_height - node_h);
                    s.layout.nodes[idx].x = new_x;
                    s.layout.nodes[idx].y = new_y;
                    s.redraw();
                } else if s.panning {
                    let dx = mx - s.pan_start_x;
                    let dy = my - s.pan_start_y;
                    s.pan_x = s.pan_start_pan_x + dx;
                    s.pan_y = s.pan_start_pan_y + dy;
                    s.redraw();
                }
            },
        );
        add_listener!("touchmove", closure);
    }

    // touchend — mirrors mouseup logic (click detection + drag end)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(web_sys::TouchEvent)>::new(
            move |event: web_sys::TouchEvent| {
                event.prevent_default();
                // Use changedTouches for the finger that was lifted
                let touch = event.changed_touches().get(0);
                let mut s = state.borrow_mut();

                if let Some(touch) = touch {
                    let rect = s.canvas.get_bounding_client_rect();
                    let mx = touch.client_x() as f64 - rect.left();
                    let my = touch.client_y() as f64 - rect.top();

                    let is_click = s
                        .mouse_down_pos
                        .map(|(dx, dy)| {
                            ((mx - dx).powi(2) + (my - dy).powi(2)).sqrt() < CLICK_THRESHOLD
                        })
                        .unwrap_or(false);

                    if is_click {
                        if let Some(idx) = s.hit_test(mx, my) {
                            let job_id = s.layout.nodes[idx].job_id.clone();
                            s.selected.clear();
                            s.selected.insert(job_id.clone());
                            s.fire_selection_change();
                            if let Some(ref cb) = s.on_node_click {
                                cb.call1(&JsValue::NULL, &JsValue::from_str(&job_id)).ok();
                            }
                            s.redraw();
                        } else {
                            if !s.selected.is_empty() {
                                s.selected.clear();
                                s.fire_selection_change();
                                s.redraw();
                            }
                            if let Some(ref cb) = s.on_canvas_click {
                                cb.call0(&JsValue::NULL).ok();
                            }
                        }
                    }

                    if let Some(idx) = s.dragging
                        && !is_click
                        && let Some(ref cb) = s.on_node_drag_end
                    {
                        let node = &s.layout.nodes[idx];
                        let _ = cb.call3(
                            &JsValue::NULL,
                            &JsValue::from_str(&node.job_id),
                            &JsValue::from_f64(node.x),
                            &JsValue::from_f64(node.y),
                        );
                    }
                }

                s.mouse_down_pos = None;
                s.dragging = None;
                s.panning = false;
            },
        );
        add_listener!("touchend", closure);
    }

    Ok(listeners)
}

fn touch_pos(touch: &web_sys::Touch, state: &SharedState) -> (f64, f64) {
    let s = state.borrow();
    let rect = s.canvas.get_bounding_client_rect();
    (
        touch.client_x() as f64 - rect.left(),
        touch.client_y() as f64 - rect.top(),
    )
}

fn mouse_pos(event: &MouseEvent, state: &SharedState) -> (f64, f64) {
    let s = state.borrow();
    let rect = s.canvas.get_bounding_client_rect();
    (
        event.client_x() as f64 - rect.left(),
        event.client_y() as f64 - rect.top(),
    )
}

/// Evaluate a cubic bezier at parameter t ∈ [0,1].
fn bezier_point(
    x0: f64, y0: f64,
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    x3: f64, y3: f64,
    t: f64,
) -> (f64, f64) {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    let t2 = t * t;
    let t3 = t2 * t;
    (
        mt3 * x0 + 3.0 * mt2 * t * x1 + 3.0 * mt * t2 * x2 + t3 * x3,
        mt3 * y0 + 3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3 * y3,
    )
}

/// Create a hidden ARIA live region for screen reader announcements.
fn create_live_region(
    document: &web_sys::Document,
    canvas: &HtmlCanvasElement,
) -> Result<web_sys::HtmlElement, JsValue> {
    let el = document
        .create_element("div")?
        .dyn_into::<web_sys::HtmlElement>()?;
    el.set_attribute("aria-live", "polite")?;
    el.set_attribute("aria-atomic", "true")?;
    el.set_attribute("role", "status")?;
    // Visually hidden but accessible to screen readers
    let style = el.style();
    style.set_property("position", "absolute")?;
    style.set_property("width", "1px")?;
    style.set_property("height", "1px")?;
    style.set_property("overflow", "hidden")?;
    style.set_property("clip", "rect(0 0 0 0)")?;
    style.set_property("white-space", "nowrap")?;

    // Insert after the canvas
    if let Some(parent) = canvas.parent_element() {
        parent
            .insert_before(&el, canvas.next_sibling().as_ref())
            .ok();
    }
    Ok(el)
}
