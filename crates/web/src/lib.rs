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
use workflow_graph_shared::{Job, JobStatus, Port, PortDirection, Workflow};

const CLICK_THRESHOLD: f64 = 5.0;
const MIN_ZOOM: f64 = 0.25;
const MAX_ZOOM: f64 = 4.0;
const ZOOM_SPEED: f64 = 0.001;

/// State for dragging from a port to create a new edge.
struct PortDragState {
    from_node_id: String,
    from_port_id: String,
    _from_port_type: String,
    from_is_output: bool,
    /// Current mouse position in graph-space.
    current_x: f64,
    current_y: f64,
}

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
    /// Rubber-band selection rectangle in graph-space: (start_x, start_y, current_x, current_y)
    selection_rect: Option<(f64, f64, f64, f64)>,
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
    on_drop: Option<js_sys::Function>,
    on_connect: Option<js_sys::Function>,
    on_field_click: Option<js_sys::Function>,
    // Node type definitions registry
    node_definitions: HashMap<String, workflow_graph_shared::NodeDefinition>,
    // Port connection dragging state
    port_dragging: Option<PortDragState>,
    // Click detection
    mouse_down_pos: Option<(f64, f64)>,
    // Resize
    auto_resize: bool,
    _resize_observer: Option<ResizeObserver>,
    // Accessibility
    live_region: Option<web_sys::HtmlElement>,
    last_announced: String,
    // Lifecycle
    destroyed: bool,
}

impl GraphState {
    fn redraw_with_time(&self, animation_time: f64, now_ms: f64) {
        if self.destroyed {
            return;
        }
        // When autoResize is on, always use the parent container size
        // so nodes can be rendered anywhere without clipping
        let (tw, th) = if self.auto_resize {
            if let Some(parent) = self.canvas.parent_element() {
                let rect = parent.get_bounding_client_rect();
                let pw = rect.width();
                let ph = rect.height();
                if pw > 0.0 && ph > 0.0 {
                    (pw, ph)
                } else {
                    (self.canvas_width, self.canvas_height)
                }
            } else {
                (self.canvas_width, self.canvas_height)
            }
        } else {
            (self.canvas_width, self.canvas_height)
        };
        // Guard against zero dimensions
        if tw <= 0.0 || th <= 0.0 {
            return;
        }
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
                node_definitions: &self.node_definitions,
                selection_rect: self.selection_rect,
            },
            self.port_dragging
                .as_ref()
                .map(|pd| {
                    // Find the port's screen position from the node layout
                    let (start_x, start_y) =
                        self.port_screen_pos(&pd.from_node_id, &pd.from_port_id, pd.from_is_output);
                    render::PortDragRender {
                        from_x: start_x,
                        from_y: start_y,
                        to_x: pd.current_x,
                        to_y: pd.current_y,
                        color: "#58a6ff".to_string(),
                    }
                })
                .as_ref(),
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

    /// Get the graph-space position of a port on a node.
    fn port_screen_pos(&self, node_id: &str, port_id: &str, is_output: bool) -> (f64, f64) {
        use workflow_graph_shared::PortDirection;
        let port_radius: f64 = 5.0;

        if let Some(node) = self.layout.nodes.iter().find(|n| n.job_id == node_id)
            && let Some(job) = self.workflow.jobs.iter().find(|j| j.id == node_id)
        {
            let ports_of_dir: Vec<_> = job
                .ports
                .iter()
                .filter(|p| {
                    if is_output {
                        p.direction == PortDirection::Output
                    } else {
                        p.direction == PortDirection::Input
                    }
                })
                .collect();

            if let Some(idx) = ports_of_dir.iter().position(|p| p.id == port_id) {
                let px = if is_output {
                    node.x + node.width
                } else {
                    node.x
                };
                let py = node.y + port_y_offset(idx, ports_of_dir.len(), node.height, port_radius);
                return (px, py);
            }
        }
        (0.0, 0.0)
    }

    /// Hit-test ports: returns (node_index, port_id, is_output, port_type, port_center_x, port_center_y).
    fn port_hit_test(&self, gx: f64, gy: f64) -> Option<(usize, String, bool, String, f64, f64)> {
        use workflow_graph_shared::PortDirection;
        let port_radius: f64 = 5.0;
        let port_hit_radius: f64 = 6.0; // tight to avoid intercepting node drags

        for (node_idx, node) in self.layout.nodes.iter().enumerate() {
            if let Some(job) = self.workflow.jobs.iter().find(|j| j.id == node.job_id) {
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

                // Input ports on the left edge
                for (i, port) in input_ports.iter().enumerate() {
                    let px = node.x;
                    let py = node.y + port_y_offset(i, input_ports.len(), node.height, port_radius);
                    if (gx - px).powi(2) + (gy - py).powi(2) < port_hit_radius.powi(2) {
                        return Some((
                            node_idx,
                            port.id.clone(),
                            false,
                            port.port_type.clone(),
                            px,
                            py,
                        ));
                    }
                }

                // Output ports on the right edge
                for (i, port) in output_ports.iter().enumerate() {
                    let px = node.x + node.width;
                    let py =
                        node.y + port_y_offset(i, output_ports.len(), node.height, port_radius);
                    if (gx - px).powi(2) + (gy - py).powi(2) < port_hit_radius.powi(2) {
                        return Some((
                            node_idx,
                            port.id.clone(),
                            true,
                            port.port_type.clone(),
                            px,
                            py,
                        ));
                    }
                }
            }
        }
        None
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
            if let Some(old) = old_statuses.get(&job.id)
                && *old != job.status
            {
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
    console_error_panic_hook::set_once();
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
#[allow(clippy::too_many_arguments)]
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
        Some(ref json) if !json.is_empty() => match serde_json::from_str(json) {
            Ok(tc) => Some(tc),
            Err(e) => {
                web_sys::console::warn_1(&JsValue::from_str(&format!(
                    "Theme JSON parse warning (using defaults): {e}"
                )));
                None
            }
        },
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

    // Set canvas CSS to fill its container
    let html_el: &HtmlElement = canvas.unchecked_ref();
    html_el.style().set_property("width", "100%").ok();
    html_el.style().set_property("height", "100%").ok();
    html_el.style().set_property("display", "block").ok();

    // Use parent container size if available, otherwise tight layout dimensions
    let (initial_w, initial_h) = {
        let rect = canvas.get_bounding_client_rect();
        let cw = rect.width();
        let ch = rect.height();
        if cw > 0.0 && ch > 0.0 {
            (cw, ch)
        } else if let Some(parent) = canvas.parent_element() {
            let prect = parent.get_bounding_client_rect();
            let pw = prect.width();
            let ph = prect.height();
            if pw > 0.0 && ph > 0.0 {
                (pw, ph)
            } else {
                (
                    graph_layout.total_width.max(600.0),
                    graph_layout.total_height.max(300.0),
                )
            }
        } else {
            (
                graph_layout.total_width.max(600.0),
                graph_layout.total_height.max(300.0),
            )
        }
    };

    let state = Rc::new(RefCell::new(GraphState {
        workflow,
        canvas_width: initial_w,
        canvas_height: initial_h,
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
        selection_rect: None,
        animating: false,
        on_node_click,
        on_node_hover,
        on_canvas_click,
        on_selection_change,
        on_node_drag_end,
        on_edge_click: None,
        on_render_node: None,
        on_drop: None,
        on_connect: None,
        on_field_click: None,
        node_definitions: HashMap::new(),
        port_dragging: None,
        mouse_down_pos: None,
        auto_resize: false,
        _resize_observer: None,
        live_region: Some(live_region),
        last_announced: String::new(),
        destroyed: false,
    }));

    state.borrow().redraw();
    let listeners = attach_event_handlers(&canvas, &state)?;

    // Listen for DPR changes (e.g., window moved between displays)
    {
        let state = state.clone();
        let dpr = window.device_pixel_ratio();
        let query = format!("(resolution: {dpr}dppx)");
        if let Ok(mql) = window.match_media(&query)
            && let Some(mql) = mql
        {
            let closure = Closure::<dyn FnMut()>::new(move || {
                if let Some(win) = web_sys::window() {
                    let new_dpr = win.device_pixel_ratio();
                    if let Ok(mut s) = state.try_borrow_mut()
                        && (s.dpr - new_dpr).abs() > 0.01
                    {
                        s.dpr = new_dpr;
                        s.redraw();
                    }
                }
            });
            mql.add_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()))
                .ok();
            closure.forget(); // Intentional: lives for the lifetime of the page
        }
    }

    let id = canvas_id.to_string();
    let instance = GraphInstance {
        state: state.clone(),
        listeners,
    };
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
            let Ok(mut s) = state.try_borrow_mut() else {
                return Ok(());
            };
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
            let has_running = s.has_running_jobs();
            let already_animating = s.animating;
            // If the animation loop is running, it handles redraws via requestAnimationFrame.
            // Only do a static redraw when not animating.
            if s.dragging.is_none() && !(has_running && already_animating) {
                s.redraw();
            }
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
    let theme_config: theme::ThemeConfig = match serde_json::from_str(theme_json) {
        Ok(tc) => tc,
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "Theme JSON parse warning (using defaults): {e}"
            )));
            return Ok(());
        }
    };
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
            let new_layout =
                layout::compute_layout_with_defs(&s.workflow, &s.theme, &s.node_definitions);
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
            let Ok(mut s) = state.try_borrow_mut() else {
                return Ok(());
            };
            if enabled && !s.auto_resize {
                let parent = s
                    .canvas
                    .parent_element()
                    .ok_or_else(|| JsValue::from_str("canvas has no parent element"))?;

                let state_clone = state.clone();
                let closure = Closure::<dyn FnMut(js_sys::Array, ResizeObserver)>::new(
                    move |entries: js_sys::Array, _observer: ResizeObserver| {
                        // Guard: if state is already borrowed or destroyed, skip
                        let Ok(mut s) = state_clone.try_borrow_mut() else {
                            return;
                        };
                        if s.destroyed {
                            return;
                        }
                        let entry: ResizeObserverEntry = match entries.get(0).dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let rect = entry.content_rect();
                        let w = rect.width();
                        let h = rect.height();
                        if w > 0.0 && h > 0.0 {
                            s.canvas_width = w;
                            s.canvas_height = h;
                            s.redraw();
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

/// Force a redraw of the graph. Used by JS-side ResizeObserver.
#[wasm_bindgen]
pub fn redraw(canvas_id: &str) {
    with_state(canvas_id, |s| {
        if !s.destroyed {
            s.redraw();
        }
    });
}

/// Resize the canvas and redraw. Called by JS-side ResizeObserver.
#[wasm_bindgen]
pub fn resize_canvas(canvas_id: &str, width: f64, height: f64) {
    with_state(canvas_id, |s| {
        if !s.destroyed && width > 0.0 && height > 0.0 {
            s.canvas_width = width;
            s.canvas_height = height;
            s.redraw();
        }
    });
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
            // Return JSON string — serde_wasm_bindgen produces Map not Object
            match serde_json::to_string(&positions) {
                Ok(json) => JsValue::from_str(&json),
                Err(_) => JsValue::NULL,
            }
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

/// Set a connect callback: `(fromNodeId: string, fromPortId: string, toNodeId: string, toPortId: string) => void`.
/// Called when the user drags from an output port to an input port to create a connection.
#[wasm_bindgen]
pub fn set_on_connect(canvas_id: &str, callback: js_sys::Function) {
    with_state(canvas_id, |s| {
        s.on_connect = Some(callback);
    });
}

/// Set a drop callback: `(x: number, y: number, data: string) => void`.
/// Called when an external element is dropped on the canvas.
/// `x` and `y` are graph-space coordinates, `data` is the dataTransfer text.
#[wasm_bindgen]
pub fn set_on_drop(canvas_id: &str, callback: js_sys::Function) {
    with_state(canvas_id, |s| {
        s.on_drop = Some(callback);
    });
}

/// Set a field click callback: `(nodeId: string, fieldKey: string, screenX: number, screenY: number) => void`.
#[wasm_bindgen]
pub fn set_on_field_click(canvas_id: &str, callback: js_sys::Function) {
    with_state(canvas_id, |s| {
        s.on_field_click = Some(callback);
    });
}

/// Register a node type definition (JSON). Used for production node rendering.
#[wasm_bindgen]
pub fn register_node_type(canvas_id: &str, def_json: &str) -> Result<(), JsValue> {
    let def: workflow_graph_shared::NodeDefinition = serde_json::from_str(def_json)
        .map_err(|e| JsValue::from_str(&format!("NodeDefinition JSON parse error: {e}")))?;
    with_state(canvas_id, |s| {
        s.node_definitions.insert(def.node_type.clone(), def);
        s.redraw();
    });
    Ok(())
}

// ─── Node CRUD API ───────────────────────────────────────────────────────────

/// Add a new node (job) to the graph. Optionally specify position (x, y).
/// If x/y are not provided (NaN or negative), positions below existing nodes.
#[wasm_bindgen]
pub fn add_node(
    canvas_id: &str,
    job_json: &str,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), JsValue> {
    let job: Job = serde_json::from_str(job_json)
        .map_err(|e| JsValue::from_str(&format!("Job JSON parse error: {e}")))?;

    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            // Prevent duplicate IDs
            if s.workflow.jobs.iter().any(|j| j.id == job.id) {
                return Err(JsValue::from_str(&format!(
                    "Node with id '{}' already exists",
                    job.id
                )));
            }
            // Copy layout values to avoid borrow conflict
            let padding = s.theme.layout.padding;
            let v_gap = s.theme.layout.v_gap;
            let node_width = s.theme.layout.node_width;
            let node_height = s.theme.layout.node_height;

            // Use provided coordinates or auto-position below existing nodes
            let (new_x, new_y) = match (x, y) {
                (Some(px), Some(py)) if px >= 0.0 && py >= 0.0 => (px, py),
                _ => {
                    let max_y = s
                        .layout
                        .nodes
                        .iter()
                        .map(|n| n.y + n.height)
                        .fold(0.0_f64, f64::max);
                    let auto_x = padding;
                    let auto_y = if s.layout.nodes.is_empty() {
                        padding
                    } else {
                        max_y + v_gap
                    };
                    (auto_x, auto_y)
                }
            };
            s.layout.nodes.push(layout::NodeLayout {
                job_id: job.id.clone(),
                x: new_x,
                y: new_y,
                width: node_width,
                height: node_height,
            });
            // Expand canvas if needed but never shrink
            s.canvas_width = s.canvas_width.max(new_x + node_width + padding);
            s.canvas_height = s.canvas_height.max(new_y + node_height + padding);
            s.workflow.jobs.push(job);
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Remove a node and all its connected edges. Triggers re-layout and re-render.
#[wasm_bindgen]
pub fn remove_node(canvas_id: &str, job_id: &str) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            let original_len = s.workflow.jobs.len();
            s.workflow.jobs.retain(|j| j.id != job_id);
            if s.workflow.jobs.len() == original_len {
                return Err(JsValue::from_str(&format!("No node with id '{job_id}'")));
            }
            // Remove edges referencing this node
            for job in &mut s.workflow.jobs {
                job.depends_on.retain(|dep| dep != job_id);
            }
            s.selected.remove(job_id);
            let new_layout =
                layout::compute_layout_with_defs(&s.workflow, &s.theme, &s.node_definitions);
            s.canvas_width = new_layout.total_width;
            s.canvas_height = new_layout.total_height;
            s.initial_layout = new_layout.clone();
            s.layout = new_layout;
            s.highlighted_edges.clear();
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Update a node's properties (partial update via JSON merge).
#[wasm_bindgen]
pub fn update_node(canvas_id: &str, job_id: &str, partial_json: &str) -> Result<(), JsValue> {
    let partial: serde_json::Value = serde_json::from_str(partial_json)
        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {e}")))?;

    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            let job = s
                .workflow
                .jobs
                .iter_mut()
                .find(|j| j.id == job_id)
                .ok_or_else(|| JsValue::from_str(&format!("No node with id '{job_id}'")))?;

            // Merge fields from the partial JSON
            if let Some(name) = partial.get("name").and_then(|v| v.as_str()) {
                job.name = name.to_string();
            }
            if let Some(status) = partial.get("status").and_then(|v| v.as_str()) {
                job.status = match status {
                    "queued" => JobStatus::Queued,
                    "running" => JobStatus::Running,
                    "success" => JobStatus::Success,
                    "failure" => JobStatus::Failure,
                    "skipped" => JobStatus::Skipped,
                    "cancelled" => JobStatus::Cancelled,
                    _ => job.status.clone(),
                };
            }
            if let Some(command) = partial.get("command").and_then(|v| v.as_str()) {
                job.command = command.to_string();
            }
            if let Some(metadata) = partial.get("metadata").and_then(|v| v.as_object()) {
                for (k, v) in metadata {
                    job.metadata.insert(k.clone(), v.clone());
                }
            }

            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Add an edge between two nodes, optionally specifying ports.
/// If from_port/to_port are provided, the edge connects specific ports.
#[wasm_bindgen]
pub fn add_edge(
    canvas_id: &str,
    from_id: &str,
    to_id: &str,
    from_port: Option<String>,
    to_port: Option<String>,
    metadata_json: Option<String>,
) -> Result<(), JsValue> {
    let edge_metadata: std::collections::HashMap<String, serde_json::Value> = match metadata_json {
        Some(ref json) if !json.is_empty() => serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Metadata JSON parse error: {e}")))?,
        _ => std::collections::HashMap::new(),
    };

    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            // Validate both nodes exist
            let from_exists = s.workflow.jobs.iter().any(|j| j.id == from_id);
            let to_exists = s.workflow.jobs.iter().any(|j| j.id == to_id);
            if !from_exists {
                return Err(JsValue::from_str(&format!(
                    "Source node '{from_id}' not found"
                )));
            }
            if !to_exists {
                return Err(JsValue::from_str(&format!(
                    "Target node '{to_id}' not found"
                )));
            }
            // Add dependency (edge is from_id -> to_id, meaning to_id depends on from_id)
            let to_job = s
                .workflow
                .jobs
                .iter_mut()
                .find(|j| j.id == to_id)
                .expect("validated above");
            if !to_job.depends_on.contains(&from_id.to_string()) {
                to_job.depends_on.push(from_id.to_string());
            }
            // Add to layout edges
            s.layout.edges.push(layout::Edge {
                from_id: from_id.to_string(),
                to_id: to_id.to_string(),
                from_port: from_port.unwrap_or_default(),
                to_port: to_port.unwrap_or_default(),
                metadata: edge_metadata,
            });
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Remove an edge between two nodes. Triggers re-render.
#[wasm_bindgen]
pub fn remove_edge(canvas_id: &str, from_id: &str, to_id: &str) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            // Remove dependency
            if let Some(to_job) = s.workflow.jobs.iter_mut().find(|j| j.id == to_id) {
                to_job.depends_on.retain(|dep| dep != from_id);
            }
            // Remove from layout edges
            s.layout
                .edges
                .retain(|e| !(e.from_id == from_id && e.to_id == to_id));
            s.highlighted_edges.clear();
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Get all nodes as a JSON array of Job objects.
#[wasm_bindgen]
pub fn get_nodes(canvas_id: &str) -> JsValue {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let s = instance.state.borrow();
            serde_wasm_bindgen::to_value(&s.workflow.jobs).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    })
}

/// Get all edges as a JSON array of `{ from_id, to_id, metadata }` objects.
#[wasm_bindgen]
pub fn get_edges(canvas_id: &str) -> JsValue {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let s = instance.state.borrow();
            let edges: Vec<serde_json::Value> = s
                .layout
                .edges
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "from_id": e.from_id,
                        "to_id": e.to_id,
                        "from_port": e.from_port,
                        "to_port": e.to_port,
                        "metadata": e.metadata,
                    })
                })
                .collect();
            serde_wasm_bindgen::to_value(&edges).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    })
}

// ─── Compound Node API ───────────────────────────────────────────────────────

/// Group selected nodes into a compound node.
/// Creates a new node with the selected nodes as children.
/// The compound node's ports are the unconnected external ports of the children.
#[wasm_bindgen]
pub fn group_selected(canvas_id: &str, group_name: &str) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            if s.destroyed {
                return Ok(());
            }

            let selected_ids: Vec<String> = s.selected.iter().cloned().collect();
            if selected_ids.len() < 2 {
                return Err(JsValue::from_str("Select at least 2 nodes to group"));
            }

            // Extract children from workflow
            let mut children: Vec<Job> = Vec::new();
            let mut remaining: Vec<Job> = Vec::new();
            for job in s.workflow.jobs.drain(..) {
                if selected_ids.contains(&job.id) {
                    children.push(job);
                } else {
                    remaining.push(job);
                }
            }

            // Collect all child IDs for edge filtering (owned to avoid borrow conflict)
            let child_ids: std::collections::HashSet<String> =
                children.iter().map(|j| j.id.clone()).collect();

            // Find the first (entry) and last (exit) nodes by x-position in layout.
            // First = leftmost (receives external input), Last = rightmost (produces external output).
            let first_id = children
                .iter()
                .min_by(|a, b| {
                    let ax = s
                        .layout
                        .nodes
                        .iter()
                        .find(|n| n.job_id == a.id)
                        .map(|n| n.x)
                        .unwrap_or(0.0);
                    let bx = s
                        .layout
                        .nodes
                        .iter()
                        .find(|n| n.job_id == b.id)
                        .map(|n| n.x)
                        .unwrap_or(0.0);
                    ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|j| j.id.clone())
                .unwrap_or_default();
            let last_id = children
                .iter()
                .max_by(|a, b| {
                    let ax = s
                        .layout
                        .nodes
                        .iter()
                        .find(|n| n.job_id == a.id)
                        .map(|n| n.x)
                        .unwrap_or(0.0);
                    let bx = s
                        .layout
                        .nodes
                        .iter()
                        .find(|n| n.job_id == b.id)
                        .map(|n| n.x)
                        .unwrap_or(0.0);
                    ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|j| j.id.clone())
                .unwrap_or_default();

            // Group ports: inputs from the first node, outputs from the last node
            let mut group_input_ports: Vec<Port> = Vec::new();
            let mut group_output_ports: Vec<Port> = Vec::new();

            for child in &children {
                for port in &child.ports {
                    match port.direction {
                        PortDirection::Input if child.id == first_id => {
                            group_input_ports.push(Port {
                                id: format!("{}.{}", child.id, port.id),
                                label: port.label.clone(),
                                direction: PortDirection::Input,
                                port_type: port.port_type.clone(),
                                color: port.color.clone(),
                            });
                        }
                        PortDirection::Output if child.id == last_id => {
                            group_output_ports.push(Port {
                                id: format!("{}.{}", child.id, port.id),
                                label: port.label.clone(),
                                direction: PortDirection::Output,
                                port_type: port.port_type.clone(),
                                color: port.color.clone(),
                            });
                        }
                        _ => {}
                    }
                }
            }

            // Calculate group position (center of children)
            let child_positions: Vec<(f64, f64)> = selected_ids
                .iter()
                .filter_map(|id| {
                    s.layout
                        .nodes
                        .iter()
                        .find(|n| n.job_id == *id)
                        .map(|n| (n.x, n.y))
                })
                .collect();
            let avg_x = child_positions.iter().map(|(x, _)| x).sum::<f64>()
                / child_positions.len().max(1) as f64;
            let avg_y = child_positions.iter().map(|(_, y)| y).sum::<f64>()
                / child_positions.len().max(1) as f64;

            // Create compound node
            let group_id = format!("group-{}", js_sys::Date::now() as u64);
            let mut all_ports = group_input_ports;
            all_ports.extend(group_output_ports);

            let compound = Job {
                id: group_id.clone(),
                name: group_name.to_string(),
                status: JobStatus::Queued,
                command: String::new(),
                duration_secs: None,
                started_at: None,
                depends_on: vec![],
                output: None,
                required_labels: vec![],
                max_retries: 0,
                attempt: 0,
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("node_type".to_string(), serde_json::json!("group"));
                    m
                },
                ports: all_ports,
                children: Some(children),
                collapsed: true,
            };

            remaining.push(compound);
            s.workflow.jobs = remaining;

            // Remove child nodes from layout, add group node
            s.layout.nodes.retain(|n| !selected_ids.contains(&n.job_id));

            let node_w = s.theme.layout.node_width;
            let node_h = s.theme.layout.node_height;
            s.layout.nodes.push(layout::NodeLayout {
                job_id: group_id,
                x: avg_x,
                y: avg_y,
                width: node_w,
                height: node_h,
            });

            // Remove internal edges (between children), keep external
            s.layout.edges.retain(|e| {
                !(child_ids.contains(e.from_id.as_str()) && child_ids.contains(e.to_id.as_str()))
            });

            s.selected.clear();
            s.fire_selection_change();
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Ungroup a compound node, restoring its children to the canvas.
#[wasm_bindgen]
pub fn ungroup_node(canvas_id: &str, node_id: &str) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            if s.destroyed {
                return Ok(());
            }

            // Find the compound node
            let idx = s.workflow.jobs.iter().position(|j| j.id == node_id);
            let Some(idx) = idx else {
                return Err(JsValue::from_str(&format!("No node '{node_id}'")));
            };

            let children = s.workflow.jobs[idx].children.clone();
            let Some(children) = children else {
                return Err(JsValue::from_str("Node is not a compound node"));
            };

            // Get group position for placing children
            let group_pos = s
                .layout
                .nodes
                .iter()
                .find(|n| n.job_id == node_id)
                .map(|n| (n.x, n.y))
                .unwrap_or((30.0, 30.0));

            // Remove compound node
            s.workflow.jobs.remove(idx);
            s.layout.nodes.retain(|n| n.job_id != node_id);

            // Add children back
            let tl_w = s.theme.layout.node_width;
            let tl_h = s.theme.layout.node_height;
            let v_gap = s.theme.layout.v_gap;

            for (i, child) in children.into_iter().enumerate() {
                let child_id = child.id.clone();
                s.workflow.jobs.push(child);
                s.layout.nodes.push(layout::NodeLayout {
                    job_id: child_id,
                    x: group_pos.0,
                    y: group_pos.1 + (i as f64) * (tl_h + v_gap),
                    width: tl_w,
                    height: tl_h,
                });
            }

            s.selected.clear();
            s.fire_selection_change();
            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Toggle a compound node between collapsed and expanded state.
#[wasm_bindgen]
pub fn toggle_collapse(canvas_id: &str, node_id: &str) -> Result<(), JsValue> {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();
            if s.destroyed {
                return Ok(());
            }

            if let Some(job) = s.workflow.jobs.iter_mut().find(|j| j.id == node_id)
                && job.children.is_some()
            {
                job.collapsed = !job.collapsed;
                s.redraw();
            }
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Get the full graph state as a JSON string for persistence.
/// Includes workflow data, node positions, zoom, pan, and selection.
/// Returns a JSON string (not a JsValue object) to avoid serde_wasm_bindgen Map issues.
#[wasm_bindgen]
pub fn get_state(canvas_id: &str) -> JsValue {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let s = instance.state.borrow();
            let positions: std::collections::HashMap<&str, (f64, f64)> = s
                .layout
                .nodes
                .iter()
                .map(|n| (n.job_id.as_str(), (n.x, n.y)))
                .collect();
            let edges: Vec<serde_json::Value> = s
                .layout
                .edges
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "from_id": e.from_id,
                        "to_id": e.to_id,
                        "from_port": e.from_port,
                        "to_port": e.to_port,
                        "metadata": e.metadata,
                    })
                })
                .collect();
            let state = serde_json::json!({
                "version": 1,
                "workflow": s.workflow,
                "positions": positions,
                "edges": edges,
                "zoom": s.zoom,
                "pan_x": s.pan_x,
                "pan_y": s.pan_y,
            });
            // Return as JSON string — JS side will JSON.parse it.
            // serde_wasm_bindgen::to_value produces Map objects instead of plain Objects.
            match serde_json::to_string(&state) {
                Ok(json) => JsValue::from_str(&json),
                Err(_) => JsValue::NULL,
            }
        } else {
            JsValue::NULL
        }
    })
}

/// Load a previously saved graph state from a JSON string.
/// Restores workflow data, node positions, zoom, and pan.
#[wasm_bindgen]
pub fn load_state(canvas_id: &str, state_json: &str) -> Result<(), JsValue> {
    let state_val: serde_json::Value = serde_json::from_str(state_json)
        .map_err(|e| JsValue::from_str(&format!("State JSON parse error: {e}")))?;

    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(instance) = graphs.get(canvas_id) {
            let mut s = instance.state.borrow_mut();

            // Restore workflow
            if let Some(workflow_val) = state_val.get("workflow")
                && let Ok(workflow) = serde_json::from_value::<Workflow>(workflow_val.clone())
            {
                s.workflow = workflow;
            }

            // Recompute layout from workflow
            let new_layout =
                layout::compute_layout_with_defs(&s.workflow, &s.theme, &s.node_definitions);
            s.layout = new_layout;

            // Restore positions (overrides computed layout)
            if let Some(positions) = state_val.get("positions").and_then(|v| v.as_object()) {
                for node in &mut s.layout.nodes {
                    if let Some(pos) = positions.get(&node.job_id)
                        && let Some(arr) = pos.as_array()
                        && arr.len() == 2
                        && let (Some(x), Some(y)) = (arr[0].as_f64(), arr[1].as_f64())
                    {
                        node.x = x;
                        node.y = y;
                    }
                }
            }

            // Restore edges with port info
            if let Some(edges_val) = state_val.get("edges").and_then(|v| v.as_array()) {
                s.layout.edges.clear();
                for edge_val in edges_val {
                    let from_id = edge_val
                        .get("from_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let to_id = edge_val
                        .get("to_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let from_port = edge_val
                        .get("from_port")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let to_port = edge_val
                        .get("to_port")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let metadata = edge_val
                        .get("metadata")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    s.layout.edges.push(layout::Edge {
                        from_id,
                        to_id,
                        from_port,
                        to_port,
                        metadata,
                    });
                }
            }

            // Restore zoom and pan
            if let Some(zoom) = state_val.get("zoom").and_then(|v| v.as_f64()) {
                s.zoom = zoom;
            }
            if let Some(pan_x) = state_val.get("pan_x").and_then(|v| v.as_f64()) {
                s.pan_x = pan_x;
            }
            if let Some(pan_y) = state_val.get("pan_y").and_then(|v| v.as_f64()) {
                s.pan_y = pan_y;
            }

            s.redraw();
            Ok(())
        } else {
            Err(JsValue::from_str(&format!(
                "No graph instance '{canvas_id}'"
            )))
        }
    })
}

/// Mark a graph instance as destroyed immediately.
/// Called synchronously from JS before the async destroy() cleanup.
/// Prevents any in-flight callbacks (animation loop, observers) from
/// accessing stale state during the async gap.
#[wasm_bindgen]
pub fn mark_destroyed(canvas_id: &str) {
    with_state(canvas_id, |s| {
        s.destroyed = true;
        if let Some(ref observer) = s._resize_observer {
            observer.disconnect();
        }
    });
}

pub fn destroy(canvas_id: &str) {
    GRAPHS.with(|g| {
        if let Some(instance) = g.borrow_mut().remove(canvas_id) {
            // Mark as destroyed first so any in-flight callbacks bail out
            if let Ok(mut s) = instance.state.try_borrow_mut() {
                s.destroyed = true;
            }
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
                    .remove_event_listener_with_callback(listener.event, &listener.js_fn)
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
        if let Some(instance) = graphs.get(canvas_id)
            && let Ok(mut s) = instance.state.try_borrow_mut()
        {
            f(&mut s);
        }
    });
}

fn maybe_start_animation(canvas_id: &str, state: &SharedState) {
    let Ok(s) = state.try_borrow() else { return };
    if !s.has_running_jobs() || s.animating {
        return;
    }
    drop(s);

    let Ok(mut s) = state.try_borrow_mut() else {
        return;
    };
    s.animating = true;
    drop(s);
    let state = state.clone();
    let _canvas_id = canvas_id.to_string();

    type AnimCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
    let callback: AnimCallback = Rc::new(RefCell::new(None));
    let callback_clone = callback.clone();

    *callback.borrow_mut() = Some(Closure::new(move |_timestamp: f64| {
        let should_continue = match state.try_borrow() {
            Ok(s) => {
                if s.destroyed || !s.has_running_jobs() {
                    false
                } else {
                    let now = js_sys::Date::now();
                    s.redraw_with_time(now / 1000.0, now);
                    true
                }
            }
            Err(_) => false, // Can't borrow → stop animation instead of continuing
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
        ($event:expr, $closure:expr, non_passive) => {{
            let closure = $closure;
            let js_fn: js_sys::Function =
                closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            canvas.add_event_listener_with_callback_and_add_event_listener_options(
                $event, &js_fn, &opts,
            )?;
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
            if state.try_borrow().map(|s| s.destroyed).unwrap_or(true) {
                return;
            }
            let (mx, my) = mouse_pos(&event, &state);
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }
            s.mouse_down_pos = Some((mx, my));

            let (gx, gy) = s.screen_to_graph(mx, my);

            // Check port hit first (for port-to-port connections)
            let has_ports = s.workflow.jobs.iter().any(|j| !j.ports.is_empty());
            if has_ports
                && let Some((node_idx, port_id, is_output, port_type, px, py)) =
                    s.port_hit_test(gx, gy)
            {
                let node_id = s.layout.nodes[node_idx].job_id.clone();
                s.port_dragging = Some(PortDragState {
                    from_node_id: node_id,
                    from_port_id: port_id,
                    _from_port_type: port_type,
                    from_is_output: is_output,
                    current_x: px,
                    current_y: py,
                });
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "crosshair").ok();
                return;
            }

            if let Some(idx) = s.hit_test(mx, my) {
                s.dragging = Some(idx);
                s.drag_offset_x = gx - s.layout.nodes[idx].x;
                s.drag_offset_y = gy - s.layout.nodes[idx].y;
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "grabbing").ok();
            } else if event.shift_key() {
                // Shift+drag on empty space → rubber-band selection
                s.selection_rect = Some((gx, gy, gx, gy));
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "crosshair").ok();
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
            if state.try_borrow().map(|s| s.destroyed).unwrap_or(true) {
                return;
            }
            let (mx, my) = mouse_pos(&event, &state);
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }

            if s.port_dragging.is_some() {
                // Cancel port drag if mouse button is no longer pressed
                if event.buttons() == 0 {
                    s.port_dragging = None;
                    let html: &HtmlElement = s.canvas.unchecked_ref();
                    html.style().set_property("cursor", "default").ok();
                    s.redraw();
                    return;
                }
                let (gx, gy) = s.screen_to_graph(mx, my);
                if let Some(ref mut pd) = s.port_dragging {
                    pd.current_x = gx;
                    pd.current_y = gy;
                }
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "crosshair").ok();
                s.redraw();
            } else if let Some(idx) = s.dragging {
                let (gx, gy) = s.screen_to_graph(mx, my);
                s.layout.nodes[idx].x = gx - s.drag_offset_x;
                s.layout.nodes[idx].y = gy - s.drag_offset_y;
                s.redraw();
            } else if s.selection_rect.is_some() {
                let (gx, gy) = s.screen_to_graph(mx, my);
                if let Some(ref mut rect) = s.selection_rect {
                    rect.2 = gx;
                    rect.3 = gy;
                }
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
            if state.try_borrow().map(|s| s.destroyed).unwrap_or(true) {
                return;
            }
            let (mx, my) = mouse_pos(&event, &state);
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }

            // Handle port connection completion
            if let Some(pd) = s.port_dragging.take() {
                let (gx, gy) = s.screen_to_graph(mx, my);
                if let Some((
                    _node_idx,
                    target_port_id,
                    target_is_output,
                    _target_port_type,
                    _px,
                    _py,
                )) = s.port_hit_test(gx, gy)
                {
                    let target_node_id = s.layout.nodes[_node_idx].job_id.clone();
                    // Must connect output→input on different nodes
                    // Type compatibility is left to the application's onConnect handler
                    let valid =
                        pd.from_is_output != target_is_output && target_node_id != pd.from_node_id;

                    if valid {
                        let (from_node, from_port, to_node, to_port) = if pd.from_is_output {
                            (
                                pd.from_node_id,
                                pd.from_port_id,
                                target_node_id,
                                target_port_id,
                            )
                        } else {
                            (
                                target_node_id,
                                target_port_id,
                                pd.from_node_id,
                                pd.from_port_id,
                            )
                        };

                        if let Some(ref cb) = s.on_connect {
                            cb.call4(
                                &JsValue::NULL,
                                &JsValue::from_str(&from_node),
                                &JsValue::from_str(&from_port),
                                &JsValue::from_str(&to_node),
                                &JsValue::from_str(&to_port),
                            )
                            .ok();
                        }
                    }
                }
                s.redraw();
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "default").ok();
                s.mouse_down_pos = None;
                return;
            }

            // Handle rubber-band selection completion
            if let Some((x1, y1, x2, y2)) = s.selection_rect.take() {
                let min_x = x1.min(x2);
                let max_x = x1.max(x2);
                let min_y = y1.min(y2);
                let max_y = y1.max(y2);

                // Select all nodes whose center is inside the rectangle
                // Collect IDs first to avoid borrow conflict
                let selected_ids: Vec<String> = s
                    .layout
                    .nodes
                    .iter()
                    .filter(|node| {
                        let cx = node.x + node.width / 2.0;
                        let cy = node.y + node.height / 2.0;
                        cx >= min_x && cx <= max_x && cy >= min_y && cy <= max_y
                    })
                    .map(|node| node.job_id.clone())
                    .collect();

                if !event.shift_key() {
                    s.selected.clear();
                }
                for id in selected_ids {
                    s.selected.insert(id);
                }
                s.fire_selection_change();
                s.redraw();
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "default").ok();
                s.mouse_down_pos = None;
                return;
            }

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
                    if let Some(ref cb) = s.on_edge_click
                        && let Some(edge_idx) = s.edge_hit_test(mx, my)
                    {
                        let edge = &s.layout.edges[edge_idx];
                        cb.call2(
                            &JsValue::NULL,
                            &JsValue::from_str(&edge.from_id),
                            &JsValue::from_str(&edge.to_id),
                        )
                        .ok();
                        edge_clicked = true;
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
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }
            let had_port_drag = s.port_dragging.is_some();
            s.dragging = None;
            s.panning = false;
            s.port_dragging = None;
            s.mouse_down_pos = None;
            let had_hover = s.hovered.is_some() || had_port_drag;
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
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }

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
        add_listener!("wheel", closure, non_passive);
    }

    // keydown (Tab to cycle nodes, Enter/Space to select, Escape to deselect)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            let Ok(mut s) = state.try_borrow_mut() else {
                return;
            };
            if s.destroyed {
                return;
            }
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
                "Delete" | "Backspace" => {
                    if !s.selected.is_empty() {
                        event.prevent_default();
                        let to_remove: Vec<String> = s.selected.iter().cloned().collect();
                        for job_id in &to_remove {
                            // Remove from workflow jobs
                            s.workflow.jobs.retain(|j| j.id != *job_id);
                            // Remove edges referencing this node
                            for job in &mut s.workflow.jobs {
                                job.depends_on.retain(|dep| dep != job_id);
                            }
                            // Remove from layout
                            s.layout.nodes.retain(|n| n.job_id != *job_id);
                            s.layout
                                .edges
                                .retain(|e| e.from_id != *job_id && e.to_id != *job_id);
                        }
                        s.selected.clear();
                        s.highlighted_edges.clear();
                        s.fire_selection_change();
                        s.redraw();
                    }
                }
                "Escape" => {
                    if !s.selected.is_empty() {
                        s.selected.clear();
                        s.fire_selection_change();
                        s.redraw();
                    }
                }
                "g" if event.ctrl_key() || event.meta_key() => {
                    // Ctrl+G / Cmd+G: Group selected nodes into compound node
                    if s.selected.len() >= 2 {
                        event.prevent_default();
                        let selected_ids: Vec<String> = s.selected.iter().cloned().collect();
                        let child_ids: std::collections::HashSet<String> =
                            selected_ids.iter().cloned().collect();

                        // Extract children
                        let mut children: Vec<Job> = Vec::new();
                        let mut remaining: Vec<Job> = Vec::new();
                        for job in s.workflow.jobs.drain(..) {
                            if child_ids.contains(&job.id) {
                                children.push(job);
                            } else {
                                remaining.push(job);
                            }
                        }

                        // Compute group position
                        let positions: Vec<(f64, f64)> = selected_ids
                            .iter()
                            .filter_map(|id| {
                                s.layout
                                    .nodes
                                    .iter()
                                    .find(|n| &n.job_id == id)
                                    .map(|n| (n.x, n.y))
                            })
                            .collect();
                        let avg_x = positions.iter().map(|p| p.0).sum::<f64>()
                            / positions.len().max(1) as f64;
                        let avg_y = positions.iter().map(|p| p.1).sum::<f64>()
                            / positions.len().max(1) as f64;

                        // Collect external ports
                        let mut ports: Vec<Port> = Vec::new();
                        for child in &children {
                            for port in &child.ports {
                                let is_external = match port.direction {
                                    PortDirection::Input => !s.layout.edges.iter().any(|e| {
                                        e.to_id == child.id
                                            && e.to_port == port.id
                                            && child_ids.contains(&e.from_id)
                                    }),
                                    PortDirection::Output => !s.layout.edges.iter().any(|e| {
                                        e.from_id == child.id
                                            && e.from_port == port.id
                                            && child_ids.contains(&e.to_id)
                                    }),
                                };
                                if is_external {
                                    ports.push(Port {
                                        id: format!("{}.{}", child.id, port.id),
                                        label: format!("{}: {}", child.name, port.label),
                                        direction: port.direction.clone(),
                                        port_type: port.port_type.clone(),
                                        color: port.color.clone(),
                                    });
                                }
                            }
                        }

                        let group_id = format!("group-{}", js_sys::Date::now() as u64);
                        let compound = Job {
                            id: group_id.clone(),
                            name: "Group".to_string(),
                            status: JobStatus::Queued,
                            command: String::new(),
                            duration_secs: None,
                            started_at: None,
                            depends_on: vec![],
                            output: None,
                            required_labels: vec![],
                            max_retries: 0,
                            attempt: 0,
                            metadata: {
                                let mut m = std::collections::HashMap::new();
                                m.insert("node_type".to_string(), serde_json::json!("group"));
                                m
                            },
                            ports,
                            children: Some(children),
                            collapsed: true,
                        };

                        remaining.push(compound);
                        s.workflow.jobs = remaining;

                        // Update layout
                        s.layout.nodes.retain(|n| !child_ids.contains(&n.job_id));
                        let nw = s.theme.layout.node_width;
                        let nh = s.theme.layout.node_height;
                        s.layout.nodes.push(layout::NodeLayout {
                            job_id: group_id,
                            x: avg_x,
                            y: avg_y,
                            width: nw,
                            height: nh,
                        });
                        s.layout.edges.retain(|e| {
                            !(child_ids.contains(&e.from_id) && child_ids.contains(&e.to_id))
                        });

                        s.selected.clear();
                        s.highlighted_edges.clear();
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
        let closure =
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                event.prevent_default();
                let Some(touch) = event.touches().get(0) else {
                    return;
                };
                let (mx, my) = touch_pos(&touch, &state);
                let Ok(mut s) = state.try_borrow_mut() else {
                    return;
                };
                if s.destroyed {
                    return;
                }
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
            });
        add_listener!("touchstart", closure, non_passive);
    }

    // touchmove — mirrors mousemove logic
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                event.prevent_default();
                let Some(touch) = event.touches().get(0) else {
                    return;
                };
                let (mx, my) = touch_pos(&touch, &state);
                let Ok(mut s) = state.try_borrow_mut() else {
                    return;
                };
                if s.destroyed {
                    return;
                }

                if let Some(idx) = s.dragging {
                    let (gx, gy) = s.screen_to_graph(mx, my);
                    s.layout.nodes[idx].x = gx - s.drag_offset_x;
                    s.layout.nodes[idx].y = gy - s.drag_offset_y;
                    s.redraw();
                } else if s.panning {
                    let dx = mx - s.pan_start_x;
                    let dy = my - s.pan_start_y;
                    s.pan_x = s.pan_start_pan_x + dx;
                    s.pan_y = s.pan_start_pan_y + dy;
                    s.redraw();
                }
            });
        add_listener!("touchmove", closure, non_passive);
    }

    // touchend — mirrors mouseup logic (click detection + drag end)
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                event.prevent_default();
                // Use changedTouches for the finger that was lifted
                let touch = event.changed_touches().get(0);
                let Ok(mut s) = state.try_borrow_mut() else {
                    return;
                };
                if s.destroyed {
                    return;
                }

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
            });
        add_listener!("touchend", closure);
    }

    // dragover — allow drop by preventing default
    {
        let closure =
            Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |event: web_sys::DragEvent| {
                event.prevent_default();
                if let Some(dt) = event.data_transfer() {
                    dt.set_drop_effect("copy");
                }
            });
        add_listener!("dragover", closure);
    }

    // drop — fire on_drop callback with graph-space coordinates and transferred data
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |event: web_sys::DragEvent| {
                event.prevent_default();
                let Ok(s) = state.try_borrow() else { return };
                let Some(ref cb) = s.on_drop else {
                    return;
                };

                // Get mouse position relative to canvas
                let rect = s.canvas.get_bounding_client_rect();
                let mx = event.client_x() as f64 - rect.left();
                let my = event.client_y() as f64 - rect.top();

                // Convert to graph-space coordinates
                let (gx, gy) = s.screen_to_graph(mx, my);

                // Get the transferred data
                let data = event
                    .data_transfer()
                    .and_then(|dt| dt.get_data("application/workflow-node").ok())
                    .unwrap_or_default();

                cb.call3(
                    &JsValue::NULL,
                    &JsValue::from_f64(gx),
                    &JsValue::from_f64(gy),
                    &JsValue::from_str(&data),
                )
                .ok();
            });
        add_listener!("drop", closure);
    }

    Ok(listeners)
}

fn touch_pos(touch: &web_sys::Touch, state: &SharedState) -> (f64, f64) {
    match state.try_borrow() {
        Ok(s) => {
            let rect = s.canvas.get_bounding_client_rect();
            (
                touch.client_x() as f64 - rect.left(),
                touch.client_y() as f64 - rect.top(),
            )
        }
        Err(_) => (0.0, 0.0),
    }
}

fn mouse_pos(event: &MouseEvent, state: &SharedState) -> (f64, f64) {
    // Use try_borrow to avoid panic when ResizeObserver has a mutable borrow
    match state.try_borrow() {
        Ok(s) => {
            let rect = s.canvas.get_bounding_client_rect();
            (
                event.client_x() as f64 - rect.left(),
                event.client_y() as f64 - rect.top(),
            )
        }
        Err(_) => (0.0, 0.0),
    }
}

/// Compute the Y offset for a port within a node.
/// Distributes ports evenly below the name area (top 28px reserved).
fn port_y_offset(index: usize, total: usize, node_height: f64, _port_radius: f64) -> f64 {
    if total == 0 {
        return node_height / 2.0;
    }
    let top_margin = 28.0;
    let usable_height = node_height - top_margin;
    let spacing = usable_height / (total as f64 + 1.0);
    top_margin + spacing * (index as f64 + 1.0)
}

/// Evaluate a cubic bezier at parameter t ∈ [0,1].
#[allow(clippy::too_many_arguments)]
fn bezier_point(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
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
