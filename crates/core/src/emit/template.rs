//! Custom post-processor macro templating (D3.4, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Provides user-customizable start/end/tool change macros with dynamic parameter substitution:
//! - `{{ tool_number }}`
//! - `{{ spindle_rpm }}`
//! - `{{ feedrate }}`
//! - `{{ max_x }}`
//! - `{{ max_y }}`
//! - `{{ max_z }}`

use serde::{Deserialize, Serialize};

/// Context parameters available for substitution in macro templates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateContext {
    pub tool_number: Option<u32>,
    pub spindle_rpm: Option<f64>,
    pub feedrate: Option<f64>,
    pub max_x: Option<f64>,
    pub max_y: Option<f64>,
    pub max_z: Option<f64>,
}

/// A configurable G-code post-processing template.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GcodeTemplate {
    pub start_macro: Option<String>,
    pub end_macro: Option<String>,
    pub tool_change_macro: Option<String>,
}

impl GcodeTemplate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the start macro with given context.
    pub fn render_start(&self, ctx: &TemplateContext) -> Option<String> {
        self.start_macro.as_ref().map(|m| render_template(m, ctx))
    }

    /// Render the end macro with given context.
    pub fn render_end(&self, ctx: &TemplateContext) -> Option<String> {
        self.end_macro.as_ref().map(|m| render_template(m, ctx))
    }

    /// Render the tool change macro with given context.
    pub fn render_tool_change(&self, ctx: &TemplateContext) -> Option<String> {
        self.tool_change_macro
            .as_ref()
            .map(|m| render_template(m, ctx))
    }
}

/// Substitute template placeholders with values from `TemplateContext`.
pub fn render_template(template: &str, ctx: &TemplateContext) -> String {
    let mut out = template.to_string();

    if let Some(tool) = ctx.tool_number {
        out = out.replace("{{ tool_number }}", &tool.to_string());
        out = out.replace("{{tool_number}}", &tool.to_string());
    }
    if let Some(rpm) = ctx.spindle_rpm {
        out = out.replace("{{ spindle_rpm }}", &format!("{rpm:.0}"));
        out = out.replace("{{spindle_rpm}}", &format!("{rpm:.0}"));
    }
    if let Some(feed) = ctx.feedrate {
        out = out.replace("{{ feedrate }}", &format!("{feed:.1}"));
        out = out.replace("{{feedrate}}", &format!("{feed:.1}"));
    }
    if let Some(mx) = ctx.max_x {
        out = out.replace("{{ max_x }}", &format!("{mx:.3}"));
        out = out.replace("{{max_x}}", &format!("{mx:.3}"));
    }
    if let Some(my) = ctx.max_y {
        out = out.replace("{{ max_y }}", &format!("{my:.3}"));
        out = out.replace("{{max_y}}", &format!("{my:.3}"));
    }
    if let Some(mz) = ctx.max_z {
        out = out.replace("{{ max_z }}", &format!("{mz:.3}"));
        out = out.replace("{{max_z}}", &format!("{mz:.3}"));
    }

    out
}
