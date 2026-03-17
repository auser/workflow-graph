mod layout;
mod render;
mod theme;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement, MouseEvent};

use github_graph_shared::{JobStatus, Workflow};
use layout::GraphLayout;

const CLICK_THRESHOLD: f64 = 5.0;

/// Persistent state for an interactive graph instance.
struct GraphState {
    workflow: Workflow,
    layout: GraphLayout,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    dpr: f64,
    canvas_width: f64,
    canvas_height: f64,
    dragging: Option<usize>,
    drag_offset_x: f64,
    drag_offset_y: f64,
    hovered: Option<usize>,
    highlighted_edges: Vec<usize>,
    animating: bool,
    /// JS callback invoked when a node is clicked (not dragged).
    on_node_click: Option<js_sys::Function>,
    /// Mouse position at mousedown, used to distinguish click from drag.
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
        for (i, node) in self.layout.nodes.iter().enumerate() {
            if x >= node.x && x <= node.x + node.width
                && y >= node.y && y <= node.y + node.height
            {
                return Some(i);
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

        let all_ids: Vec<&str> = ancestor_ids.iter().chain(descendant_ids.iter())
            .map(|s| s.as_str())
            .collect();

        for (i, edge) in self.layout.edges.iter().enumerate() {
            if all_ids.contains(&edge.from_id.as_str())
                && all_ids.contains(&edge.to_id.as_str())
            {
                self.highlighted_edges.push(i);
            }
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

#[wasm_bindgen]
pub fn render_workflow(
    canvas_id: &str,
    workflow_json: &str,
    on_node_click: Option<js_sys::Function>,
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
        layout: graph_layout,
        canvas: canvas.clone(),
        ctx,
        dpr,
        dragging: None,
        drag_offset_x: 0.0,
        drag_offset_y: 0.0,
        hovered: None,
        highlighted_edges: vec![],
        animating: false,
        on_node_click,
        mouse_down_pos: None,
    }));

    state.borrow().redraw();
    attach_mouse_handlers(&canvas, &state)?;

    let id = canvas_id.to_string();
    GRAPHS.with(|g| g.borrow_mut().insert(id.clone(), state.clone()));

    // Start animation if needed
    maybe_start_animation(&id, &state);

    Ok(())
}

/// Update workflow data without resetting node positions.
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
            render_workflow(canvas_id, workflow_json, None)
        }
    })
}

/// Start a requestAnimationFrame loop for smooth spinner animation and live timers.
fn maybe_start_animation(canvas_id: &str, state: &SharedState) {
    let s = state.borrow();
    if !s.has_running_jobs() || s.animating {
        return;
    }
    drop(s);

    state.borrow_mut().animating = true;

    let state = state.clone();
    let canvas_id = canvas_id.to_string();

    // Use a shared Rc<RefCell<Option<Closure>>> so the closure can re-schedule itself
    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let callback_clone = callback.clone();

    *callback.borrow_mut() = Some(Closure::new(move |_timestamp: f64| {
        let should_continue = {
            // Use try_borrow to avoid panicking if update_workflow_data holds a borrow
            match state.try_borrow() {
                Ok(s) => {
                    if !s.has_running_jobs() {
                        false
                    } else {
                        let now = js_sys::Date::now();
                        s.redraw_with_time(now / 1000.0, now);
                        true
                    }
                }
                Err(_) => true, // Skip this frame, try again next frame
            }
        };

        if should_continue {
            // Schedule next frame
            if let Some(window) = web_sys::window() {
                if let Some(cb) = callback_clone.borrow().as_ref() {
                    window.request_animation_frame(cb.as_ref().unchecked_ref()).ok();
                }
            }
        } else {
            state.borrow_mut().animating = false;
            // Drop the closure to clean up
            *callback_clone.borrow_mut() = None;
        }
    }));

    // Kick off the first frame
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
            // Always record mousedown position for click detection
            s.mouse_down_pos = Some((mx, my));
            if let Some(idx) = s.hit_test(mx, my) {
                s.dragging = Some(idx);
                s.drag_offset_x = mx - s.layout.nodes[idx].x;
                s.drag_offset_y = my - s.layout.nodes[idx].y;
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "grabbing").ok();
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
                let node_w = s.layout.nodes[idx].width;
                let node_h = s.layout.nodes[idx].height;
                let new_x = (mx - s.drag_offset_x).clamp(0.0, s.canvas_width - node_w);
                let new_y = (my - s.drag_offset_y).clamp(0.0, s.canvas_height - node_h);
                s.layout.nodes[idx].x = new_x;
                s.layout.nodes[idx].y = new_y;
                s.redraw();
            } else {
                let new_hover = s.hit_test(mx, my);
                let changed = new_hover != s.hovered;
                s.hovered = new_hover;

                if changed {
                    s.compute_highlighted_path(new_hover);
                    s.redraw();
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

            // Detect click: mousedown + mouseup with minimal movement
            let is_click = s.mouse_down_pos
                .map(|(dx, dy)| {
                    let dist = ((mx - dx).powi(2) + (my - dy).powi(2)).sqrt();
                    dist < CLICK_THRESHOLD
                })
                .unwrap_or(false);

            if is_click {
                if let Some(idx) = s.hit_test(mx, my) {
                    let job_id = s.layout.nodes[idx].job_id.clone();
                    if let Some(ref cb) = s.on_node_click {
                        cb.call1(&JsValue::NULL, &JsValue::from_str(&job_id)).ok();
                    }
                }
            }

            s.mouse_down_pos = None;
            if s.dragging.is_some() {
                s.dragging = None;
                let html: &HtmlElement = s.canvas.unchecked_ref();
                html.style().set_property("cursor", "default").ok();
            }
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
            s.mouse_down_pos = None;
            let had_hover = s.hovered.is_some();
            s.hovered = None;
            s.highlighted_edges.clear();
            let html: &HtmlElement = s.canvas.unchecked_ref();
            html.style().set_property("cursor", "default").ok();
            if had_hover {
                s.redraw();
            }
        });
        canvas.add_event_listener_with_callback("mouseleave", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    Ok(())
}

fn mouse_pos(event: &MouseEvent, state: &SharedState) -> (f64, f64) {
    let s = state.borrow();
    let rect = s.canvas.get_bounding_client_rect();
    let x = event.client_x() as f64 - rect.left();
    let y = event.client_y() as f64 - rect.top();
    (x, y)
}
