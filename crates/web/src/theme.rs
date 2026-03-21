use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ─── Default constants (GitHub Actions light theme) ──────────────────────────

pub const NODE_WIDTH: f64 = 200.0;
pub const NODE_HEIGHT: f64 = 40.0;
pub const NODE_RADIUS: f64 = 6.0;
pub const H_GAP: f64 = 60.0;
pub const V_GAP: f64 = 16.0;
pub const HEADER_HEIGHT: f64 = 60.0;
pub const PADDING: f64 = 40.0;
pub const JUNCTION_DOT_RADIUS: f64 = 3.5;
pub const STATUS_ICON_RADIUS: f64 = 8.0;
pub const STATUS_ICON_MARGIN: f64 = 12.0;

pub const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif";
pub const FONT_SIZE_NAME: f64 = 13.0;
pub const FONT_SIZE_DURATION: f64 = 11.0;
pub const FONT_SIZE_HEADER: f64 = 14.0;

pub const COLOR_SUCCESS: &str = "#1a7f37";
pub const COLOR_FAILURE: &str = "#cf222e";
pub const COLOR_RUNNING: &str = "#bf8700";
pub const COLOR_QUEUED: &str = "#656d76";
pub const COLOR_SKIPPED: &str = "#656d76";
pub const COLOR_CANCELLED: &str = "#656d76";

pub const COLOR_NODE_BG: &str = "#ffffff";
pub const COLOR_NODE_BORDER: &str = "#d0d7de";
pub const COLOR_EDGE: &str = "#d0d7de";
pub const COLOR_JUNCTION: &str = "#8c959f";
pub const COLOR_TEXT: &str = "#1f2328";
pub const COLOR_TEXT_SECONDARY: &str = "#656d76";
pub const COLOR_BG: &str = "#ffffff";
pub const COLOR_GRAPH_BG: &str = "#f6f8fa";
pub const COLOR_HEADER_TEXT: &str = "#1f2328";
pub const COLOR_HEADER_TRIGGER: &str = "#656d76";
pub const COLOR_HIGHLIGHT: &str = "#0969da";
pub const COLOR_SELECTED: &str = "#0969da";

// ─── Runtime-configurable theme ──────────────────────────────────────────────

/// Colors for the workflow graph. All values are CSS color strings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct ThemeColors {
    // Status colors
    pub(crate) success: String,
    pub(crate) failure: String,
    pub(crate) running: String,
    pub(crate) queued: String,
    pub(crate) skipped: String,
    pub(crate) cancelled: String,
    // Node colors
    pub(crate) node_bg: String,
    pub(crate) node_border: String,
    pub(crate) text: String,
    pub(crate) text_secondary: String,
    // Graph colors
    pub(crate) bg: String,
    pub(crate) graph_bg: String,
    pub(crate) edge: String,
    pub(crate) junction: String,
    pub(crate) highlight: String,
    pub(crate) selected: String,
    // Header colors
    pub(crate) header_text: String,
    pub(crate) header_trigger: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            success: COLOR_SUCCESS.into(),
            failure: COLOR_FAILURE.into(),
            running: COLOR_RUNNING.into(),
            queued: COLOR_QUEUED.into(),
            skipped: COLOR_SKIPPED.into(),
            cancelled: COLOR_CANCELLED.into(),
            node_bg: COLOR_NODE_BG.into(),
            node_border: COLOR_NODE_BORDER.into(),
            text: COLOR_TEXT.into(),
            text_secondary: COLOR_TEXT_SECONDARY.into(),
            bg: COLOR_BG.into(),
            graph_bg: COLOR_GRAPH_BG.into(),
            edge: COLOR_EDGE.into(),
            junction: COLOR_JUNCTION.into(),
            highlight: COLOR_HIGHLIGHT.into(),
            selected: COLOR_SELECTED.into(),
            header_text: COLOR_HEADER_TEXT.into(),
            header_trigger: COLOR_HEADER_TRIGGER.into(),
        }
    }
}

/// Font configuration for the workflow graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeFonts {
    pub family: String,
    pub size_name: f64,
    pub size_duration: f64,
    pub size_header: f64,
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            family: FONT_FAMILY.into(),
            size_name: FONT_SIZE_NAME,
            size_duration: FONT_SIZE_DURATION,
            size_header: FONT_SIZE_HEADER,
        }
    }
}

/// Node dimension and spacing configuration.
/// All fields have defaults so partial JSON works.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeLayout {
    pub node_width: f64,
    pub node_height: f64,
    pub node_radius: f64,
    pub h_gap: f64,
    pub v_gap: f64,
    pub header_height: f64,
    pub padding: f64,
    pub junction_dot_radius: f64,
    pub status_icon_radius: f64,
    pub status_icon_margin: f64,
}

impl Default for ThemeLayout {
    fn default() -> Self {
        Self {
            node_width: NODE_WIDTH,
            node_height: NODE_HEIGHT,
            node_radius: NODE_RADIUS,
            h_gap: H_GAP,
            v_gap: V_GAP,
            header_height: HEADER_HEIGHT,
            padding: PADDING,
            junction_dot_radius: JUNCTION_DOT_RADIUS,
            status_icon_radius: STATUS_ICON_RADIUS,
            status_icon_margin: STATUS_ICON_MARGIN,
        }
    }
}

/// Direction the DAG flows.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum LayoutDirection {
    /// Left to right (default — matches GitHub Actions).
    #[default]
    LeftToRight,
    /// Top to bottom.
    TopToBottom,
}

/// Edge style configuration for custom edge rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EdgeStyle {
    /// CSS color for the edge line. Falls back to theme edge color if None.
    pub color: Option<String>,
    /// Line width in px. Falls back to default (2.0) if None.
    pub width: Option<f64>,
    /// Dash pattern (e.g., [5.0, 3.0] for dashed). Empty/None = solid.
    pub dash: Option<Vec<f64>>,
}

/// Per-edge style overrides keyed by "from_id->to_id".
pub type EdgeStyleMap = std::collections::HashMap<String, EdgeStyle>;

/// Internationalization labels for status text and duration formatting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Labels {
    pub queued: String,
    pub running: String,
    pub success: String,
    pub failure: String,
    pub skipped: String,
    pub cancelled: String,
    /// Format string for minutes+seconds. {m} and {s} are placeholders.
    pub duration_minutes: String,
    /// Format string for seconds only. {s} is placeholder.
    pub duration_seconds: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            queued: "Queued".into(),
            running: "Running".into(),
            success: "Success".into(),
            failure: "Failure".into(),
            skipped: "Skipped".into(),
            cancelled: "Cancelled".into(),
            duration_minutes: "{m}m {s}s".into(),
            duration_seconds: "{s}s".into(),
        }
    }
}

impl Labels {
    pub fn format_duration(&self, secs: u64) -> String {
        if secs >= 60 {
            let m = secs / 60;
            let s = secs % 60;
            self.duration_minutes
                .replace("{m}", &m.to_string())
                .replace("{s}", &s.to_string())
        } else {
            self.duration_seconds.replace("{s}", &secs.to_string())
        }
    }
}

/// Complete theme configuration passed to render_workflow.
/// Every field is optional — omitted fields use the light-theme defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub colors: Option<ThemeColors>,
    pub fonts: Option<ThemeFonts>,
    pub layout: Option<ThemeLayout>,
    pub direction: Option<LayoutDirection>,
    pub labels: Option<Labels>,
    /// Per-edge style overrides keyed by "from_id->to_id".
    pub edge_styles: Option<EdgeStyleMap>,
    /// Show the minimap overlay.
    pub minimap: Option<bool>,
}

/// Resolved theme with no Option fields — ready for rendering.
#[derive(Clone, Debug, Default)]
pub struct ResolvedTheme {
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
    pub layout: ThemeLayout,
    pub direction: LayoutDirection,
    pub labels: Labels,
    pub edge_styles: EdgeStyleMap,
    pub minimap: bool,
}

impl ResolvedTheme {
    pub fn from_config(config: Option<ThemeConfig>) -> Self {
        match config {
            None => Self::default(),
            Some(cfg) => Self {
                colors: cfg.colors.unwrap_or_default(),
                fonts: cfg.fonts.unwrap_or_default(),
                layout: cfg.layout.unwrap_or_default(),
                direction: cfg.direction.unwrap_or_default(),
                labels: cfg.labels.unwrap_or_default(),
                edge_styles: cfg.edge_styles.unwrap_or_default(),
                minimap: cfg.minimap.unwrap_or(false),
            },
        }
    }
}

// ─── Preset themes ───────────────────────────────────────────────────────────

/// GitHub Actions dark theme preset.
pub fn dark_theme_colors() -> ThemeColors {
    ThemeColors {
        success: "#3fb950".into(),
        failure: "#f85149".into(),
        running: "#d29922".into(),
        queued: "#8b949e".into(),
        skipped: "#8b949e".into(),
        cancelled: "#8b949e".into(),
        node_bg: "#161b22".into(),
        node_border: "#30363d".into(),
        text: "#e6edf3".into(),
        text_secondary: "#8b949e".into(),
        bg: "#0d1117".into(),
        graph_bg: "#161b22".into(),
        edge: "#30363d".into(),
        junction: "#484f58".into(),
        highlight: "#58a6ff".into(),
        selected: "#58a6ff".into(),
        header_text: "#e6edf3".into(),
        header_trigger: "#8b949e".into(),
    }
}

/// WCAG AA high-contrast theme preset.
/// Uses strong color separation (4.5:1+ contrast ratios) for accessibility.
pub fn high_contrast_colors() -> ThemeColors {
    ThemeColors {
        success: "#008000".into(),
        failure: "#ff0000".into(),
        running: "#ff8c00".into(),
        queued: "#555555".into(),
        skipped: "#555555".into(),
        cancelled: "#555555".into(),
        node_bg: "#ffffff".into(),
        node_border: "#000000".into(),
        text: "#000000".into(),
        text_secondary: "#333333".into(),
        bg: "#ffffff".into(),
        graph_bg: "#f0f0f0".into(),
        edge: "#000000".into(),
        junction: "#000000".into(),
        highlight: "#0000ff".into(),
        selected: "#0000ff".into(),
        header_text: "#000000".into(),
        header_trigger: "#333333".into(),
    }
}
