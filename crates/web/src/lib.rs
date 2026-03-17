mod layout;
mod render;
mod theme;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement, KeyboardEvent, MouseEvent, WheelEvent};

use workflow_graph_shared::{JobStatus, Workflow};
use layout::GraphLayout;

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
    // Click detection
    mouse_down_pos: Option<(f64, f64)>,
}

impl GraphState {
    fn redraw_with_time(&self, animation_time: f64, now_ms: f64) {
        let tw = self.canvas_width;
        let th = self.canvas_height;
        self.canvas.set_width((tw * self.dpr) as u32);
        self.canvas.set_height((th * self.dpr) as u32);
        let html_el: &HtmlElement = self.canvas.unchecked_ref();
        html_el.style().set_property("width", &format!("{tw}px")).ok();
        html_el.style().set_property("height", &format!("{th}px")).ok();

        render::render(
            &self.ctx, &self.workflow, &self.layout, self.dpr,
            &self.highlighted_edges, tw, th, animation_time, now_ms,
            self.zoom, self.pan_x, self.pan_y, &self.selected,
        ).ok();
    }

    fn redraw(&self) {
        let now = js_sys::Date::now();
        self.redraw_with_time(now / 1000.0, now);
    }

    fn has_running_jobs(&self) -> bool {
        self.workflow.jobs.iter().any(|j| j.status == JobStatus::Running)
    }

    fn hit_test(&self, x: f64, y: f64) -> Option<usize> {
        // Transform screen coords to graph coords
        let gx = (x - self.pan_x) / self.zoom;
        let gy = (y - self.pan_y) / self.zoom;
        for (i, node) in self.layout.nodes.iter().enumerate() {
            if gx >= node.x && gx <= node.x + node.width
                && gy >= node.y && gy <= node.y + node.height
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

        let all_ids: Vec<&str> = ancestor_ids.iter().chain(descendant_ids.iter())
            .map(|s| s.as_str()).collect();

        for (i, edge) in self.layout.edges.iter().enumerate() {
            if all_ids.contains(&edge.from_id.as_str())
                && all_ids.contains(&edge.to_id.as_str())
            {
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
}

type SharedState = Rc<RefCell<GraphState>>;

thread_local! {
    static GRAPHS: RefCell<HashMap<String, SharedState>> = RefCell::new(HashMap::new());
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
#[wasm_bindgen]
pub fn render_workflow(
    canvas_id: &str,
    workflow_json: &str,
    on_node_click: Option<js_sys::Function>,
    on_node_hover: Option<js_sys::Function>,
    on_canvas_click: Option<js_sys::Function>,
    on_selection_change: Option<js_sys::Function>,
    on_node_drag_end: Option<js_sys::Function>,
) -> Result<(), JsValue> {
    let workflow: Workflow = serde_json::from_str(workflow_json)
        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {e}")))?;

    let graph_layout = layout::compute_layout(&workflow);

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no canvas element '{canvas_id}'")))?
        .dyn_into::<HtmlCanvasElement>()?;

    let dpr = window.device_pixel_ratio();
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let state = Rc::new(RefCell::new(GraphState {
        workflow,
        canvas_width: graph_layout.total_width,
        canvas_height: graph_layout.total_height,
        initial_layout: graph_layout.clone(),
        layout: graph_layout,
        canvas: canvas.clone(),
        ctx,
        dpr,
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
        mouse_down_pos: None,
    }));

    state.borrow().redraw();
    attach_mouse_handlers(&canvas, &state)?;

    let id = canvas_id.to_string();
    GRAPHS.with(|g| g.borrow_mut().insert(id.clone(), state.clone()));
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
        if let Some(state) = graphs.get(canvas_id) {
            let mut s = state.borrow_mut();
            for job in &new_workflow.jobs {
                if let Some(existing) = s.workflow.jobs.iter_mut().find(|j| j.id == job.id) {
                    existing.status = job.status.clone();
                    existing.duration_secs = job.duration_secs;
                    existing.started_at = job.started_at;
                    existing.output = job.output.clone();
                }
            }
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
            render_workflow(canvas_id, workflow_json, None, None, None, None, None)
        }
    })
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
        if let Some(state) = graphs.get(canvas_id) {
            let s = state.borrow();
            let positions: HashMap<&str, (f64, f64)> = s.layout.nodes.iter()
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

/// Set an edge click callback for a graph instance.
#[wasm_bindgen]
pub fn set_on_edge_click(canvas_id: &str, callback: js_sys::Function) {
    // Edge click detection would require hit-testing bezier curves.
    // For now, store the callback — implementation of edge hit testing
    // is a future enhancement.
    let _ = (canvas_id, callback);
}

#[wasm_bindgen]
pub fn destroy(canvas_id: &str) {
    GRAPHS.with(|g| {
        g.borrow_mut().remove(canvas_id);
    });
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn with_state(canvas_id: &str, f: impl FnOnce(&mut GraphState)) {
    GRAPHS.with(|g| {
        let graphs = g.borrow();
        if let Some(state) = graphs.get(canvas_id) {
            f(&mut state.borrow_mut());
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

    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
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
            if let Some(window) = web_sys::window() {
                if let Some(cb) = callback_clone.borrow().as_ref() {
                    window.request_animation_frame(cb.as_ref().unchecked_ref()).ok();
                }
            }
        } else {
            state.borrow_mut().animating = false;
            *callback_clone.borrow_mut() = None;
        }
    }));

    if let Some(window) = web_sys::window() {
        if let Some(cb) = callback.borrow().as_ref() {
            window.request_animation_frame(cb.as_ref().unchecked_ref()).ok();
        }
    }
}

fn attach_mouse_handlers(canvas: &HtmlCanvasElement, state: &SharedState) -> Result<(), JsValue> {
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
                // Start panning (clicked on empty space)
                s.panning = true;
                s.pan_start_x = mx;
                s.pan_start_y = my;
                s.pan_start_pan_x = s.pan_x;
                s.pan_start_pan_y = s.pan_y;
            }
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
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

                    // Fire hover callback
                    if let Some(ref cb) = s.on_node_hover {
                        let val = match new_hover {
                            Some(idx) => JsValue::from_str(&s.layout.nodes[idx].job_id),
                            None => JsValue::NULL,
                        };
                        cb.call1(&JsValue::NULL, &val).ok();
                    }
                }

                let cursor = if new_hover.is_some() { "grab" } else { "default" };
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", cursor).ok();
            }
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // mouseup
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let (mx, my) = mouse_pos(&event, &state);
            let mut s = state.borrow_mut();

            let is_click = s.mouse_down_pos
                .map(|(dx, dy)| ((mx - dx).powi(2) + (my - dy).powi(2)).sqrt() < CLICK_THRESHOLD)
                .unwrap_or(false);

            if is_click {
                if let Some(idx) = s.hit_test(mx, my) {
                    let job_id = s.layout.nodes[idx].job_id.clone();

                    // Handle selection
                    if event.shift_key() {
                        // Toggle selection
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

                    // Fire click callback
                    if let Some(ref cb) = s.on_node_click {
                        cb.call1(&JsValue::NULL, &JsValue::from_str(&job_id)).ok();
                    }
                    s.redraw();
                } else {
                    // Clicked on empty space
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

            // Fire drag end callback
            if let Some(idx) = s.dragging {
                if !is_click {
                    if let Some(ref cb) = s.on_node_drag_end {
                        let node = &s.layout.nodes[idx];
                        let _ = cb.call3(
                            &JsValue::NULL,
                            &JsValue::from_str(&node.job_id),
                            &JsValue::from_f64(node.x),
                            &JsValue::from_f64(node.y),
                        );
                    }
                }
            }

            s.mouse_down_pos = None;
            s.dragging = None;
            s.panning = false;
            let html: &HtmlElement = s.canvas.unchecked_ref();
            html.style().set_property("cursor", "default").ok();
        });
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
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
        canvas.add_event_listener_with_callback("mouseleave", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // wheel (zoom)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();
            let mut s = state.borrow_mut();

            let (mx, my) = {
                let rect = s.canvas.get_bounding_client_rect();
                (event.client_x() as f64 - rect.left(), event.client_y() as f64 - rect.top())
            };

            let old_zoom = s.zoom;
            let delta = -event.delta_y() * ZOOM_SPEED;
            s.zoom = (s.zoom * (1.0 + delta)).clamp(MIN_ZOOM, MAX_ZOOM);

            // Zoom centered on cursor position
            let scale_change = s.zoom / old_zoom;
            s.pan_x = mx - (mx - s.pan_x) * scale_change;
            s.pan_y = my - (my - s.pan_y) * scale_change;

            s.redraw();
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
        closure.forget();
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

                    // Find current focused node index
                    let current_idx = if s.selected.len() == 1 {
                        let selected_id = s.selected.iter().next().unwrap();
                        s.layout.nodes.iter().position(|n| n.job_id == *selected_id)
                    } else {
                        None
                    };

                    let next_idx = if event.shift_key() {
                        // Shift+Tab: previous
                        match current_idx {
                            Some(i) if i > 0 => i - 1,
                            _ => node_count - 1,
                        }
                    } else {
                        // Tab: next
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
        canvas.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    Ok(())
}

fn mouse_pos(event: &MouseEvent, state: &SharedState) -> (f64, f64) {
    let s = state.borrow();
    let rect = s.canvas.get_bounding_client_rect();
    (event.client_x() as f64 - rect.left(), event.client_y() as f64 - rect.top())
}
