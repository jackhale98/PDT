//! Terminal visualization using braille graphics
//!
//! Provides terminal-based visualization for tolerance chains and analysis results
//! using Unicode braille characters for graphical rendering.
//!
//! Also provides SVG and ASCII isometric 3D visualization for stackup analysis.

use drawille::Canvas;

use tdt_core::core::sdt::ChainContributor3D;
use tdt_core::entities::stackup::{ResultTorsor, Stackup};

/// Render a tolerance chain schematic
///
/// Shows components connected by joints with labels.
/// Uses box-drawing characters for the chain structure.
///
/// # Example Output
/// ```text
/// ┌────────────────────────────────────────────────────────────────────┐
/// │  ┌────┐       ┌────┐       ┌────┐       ┌────┐                     │
/// │  │CMP1│──||───│CMP2│──||───│CMP3│──||───│CMP4│ → Functional Dir   │
/// │  └────┘       └────┘       └────┘       └────┘                     │
/// └────────────────────────────────────────────────────────────────────┘
/// ```
pub fn render_chain_schematic(stackup: &Stackup) -> String {
    let mut lines = Vec::new();

    // Get contributor count
    let count = stackup.contributors.len();
    if count == 0 {
        return "  (no contributors)".to_string();
    }

    // Build component names
    let names: Vec<String> = stackup
        .contributors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            // Use feature component name if available, otherwise truncate contributor name
            if let Some(ref feat_ref) = c.feature {
                if let Some(ref cmp_name) = feat_ref.component_name {
                    truncate_str(cmp_name, 6)
                } else {
                    format!("C{}", i + 1)
                }
            } else {
                truncate_str(&c.name, 6)
            }
        })
        .collect();

    // Calculate width needed
    // Each component box: [name] (8 chars) + connector (7 chars "──||───") = 15 chars
    // But last one has no connector, just arrow and "Functional Dir" (16 chars)

    // Top border
    let content_width = std::cmp::max(count * 15 + 16, stackup.title.len() + 4);
    let border_width = content_width + 4;
    lines.push(format!("┌{}┐", "─".repeat(border_width)));

    // Title line
    lines.push(format!(
        "│  {}{}  │",
        stackup.title,
        " ".repeat(content_width - stackup.title.len())
    ));

    // Empty line
    lines.push(format!("│{}│", " ".repeat(border_width)));

    // Component top boxes
    let mut top_line = String::from("│  ");
    for _ in 0..count {
        top_line.push_str("┌──────┐");
        top_line.push_str("       ");
    }
    // Pad to border
    while top_line.len() < border_width + 1 {
        top_line.push(' ');
    }
    top_line.push('│');
    lines.push(top_line);

    // Component middle (with names and connectors)
    let mut mid_line = String::from("│  ");
    for (i, name) in names.iter().enumerate() {
        let padded = format!("{:^6}", name);
        mid_line.push_str(&format!("│{}│", padded));

        if i < count - 1 {
            // Direction indicator based on contributor direction
            let dir_char = match stackup.contributors[i].direction {
                tdt_core::entities::stackup::Direction::Positive => "→",
                tdt_core::entities::stackup::Direction::Negative => "←",
            };
            mid_line.push_str(&format!("─{}{}───", dir_char, dir_char));
        } else {
            // Last component - add functional direction arrow
            mid_line.push_str(" → ");
            if let Some(dir) = stackup.functional_direction {
                mid_line.push_str(&format!("[{:.1},{:.1},{:.1}]", dir[0], dir[1], dir[2]));
            } else {
                mid_line.push_str("Func Dir");
            }
        }
    }
    while mid_line.len() < border_width + 1 {
        mid_line.push(' ');
    }
    mid_line.push('│');
    lines.push(mid_line);

    // Component bottom boxes
    let mut bot_line = String::from("│  ");
    for _ in 0..count {
        bot_line.push_str("└──────┘");
        bot_line.push_str("       ");
    }
    while bot_line.len() < border_width + 1 {
        bot_line.push(' ');
    }
    bot_line.push('│');
    lines.push(bot_line);

    // Empty line
    lines.push(format!("│{}│", " ".repeat(border_width)));

    // Bottom border
    lines.push(format!("└{}┘", "─".repeat(border_width)));

    lines.join("\n")
}

/// Truncate string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 2 {
        s.chars().take(max_len).collect()
    } else {
        format!("{}…", s.chars().take(max_len - 1).collect::<String>())
    }
}

/// Render a deviation ellipse for the UV (XY) plane
///
/// Uses braille graphics to show the 3-sigma deviation region.
/// Scale automatically adjusts to fit the canvas.
///
/// # Example Output
/// ```text
/// UV Deviation (3σ):
///     ⠀⠀⠀⣠⠶⠶⣄⠀⠀⠀
///     ⠀⢠⠋⠀⠀⠀⠀⠙⣆⠀
///     ⠀⡇⠀⠀⠀⠀⠀⠀⢸⠀
///     ⠀⠘⣆⠀⠀⠀⠀⣠⠃⠀
///     ⠀⠀⠈⠳⠶⠶⠞⠁⠀⠀
/// ```
pub fn render_deviation_ellipse(result: &ResultTorsor, size: u32) -> String {
    // Create canvas - drawille uses 2x4 pixel chars
    let mut canvas = Canvas::new(size, size);

    // Get U and V deviation ranges (3-sigma)
    let u_range = result.u.rss_3sigma.max(0.001); // Avoid zero
    let v_range = result.v.rss_3sigma.max(0.001);

    let center_x = size / 2;
    let center_y = size / 2;

    // Scale to fit canvas (use 80% of canvas)
    let scale_x = (size as f64 * 0.4) / u_range;
    let scale_y = (size as f64 * 0.4) / v_range;

    // Draw ellipse using parametric form
    let steps = 64;
    for i in 0..steps {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / (steps as f64);

        // Point on ellipse (3σ boundary)
        let u = u_range * theta.cos();
        let v = v_range * theta.sin();

        // Transform to canvas coordinates
        let px = center_x as f64 + u * scale_x;
        let py = center_y as f64 - v * scale_y; // Y inverted

        canvas.set(px as u32, py as u32);
    }

    // Draw axes
    for i in 0..size {
        canvas.set(center_x, i); // Vertical axis
        canvas.set(i, center_y); // Horizontal axis
    }

    // Draw center point (cross)
    canvas.set(center_x, center_y);
    canvas.set(center_x - 1, center_y);
    canvas.set(center_x + 1, center_y);
    canvas.set(center_x, center_y - 1);
    canvas.set(center_x, center_y + 1);

    // Build output
    let frame = canvas.frame();
    let mut output = String::new();
    output.push_str("UV Deviation (3σ):\n");
    output.push_str(&frame);
    output.push_str(&format!("\n  U: ±{:.4}  V: ±{:.4}", u_range, v_range));

    output
}

// ============================================================================
// SVG Visualization
// ============================================================================

/// Configuration for SVG rendering
#[derive(Debug, Clone)]
pub struct SvgConfig {
    /// SVG width in pixels
    pub width: u32,
    /// SVG height in pixels
    pub height: u32,
    /// Padding around content
    pub padding: u32,
    /// Isometric angle for X projection (degrees)
    pub iso_angle_x: f64,
    /// Isometric angle for Y projection (degrees)
    pub iso_angle_y: f64,
    /// Scale factor (pixels per mm)
    pub scale: f64,
}

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            padding: 50,
            iso_angle_x: 30.0,
            iso_angle_y: 30.0,
            scale: 5.0,
        }
    }
}

/// Project 3D coordinates to 2D isometric view
fn project_isometric(x: f64, y: f64, z: f64, config: &SvgConfig) -> (f64, f64) {
    let angle_x = config.iso_angle_x.to_radians();
    let angle_y = config.iso_angle_y.to_radians();

    // Isometric projection
    // X axis goes right and slightly down
    // Y axis goes left and slightly down
    // Z axis goes up
    let px = (x * angle_x.cos() - y * angle_y.cos()) * config.scale;
    let py = (-z + x * angle_x.sin() + y * angle_y.sin()) * config.scale;

    // Offset to center of canvas
    let cx = (config.width / 2) as f64 + px;
    let cy = (config.height / 2) as f64 + py;

    (cx, cy)
}

/// Render a 3D stackup analysis as SVG
///
/// Creates an isometric view showing:
/// - Feature positions as boxes
/// - Tolerance zones as dashed rectangles
/// - Chain connections as lines
/// - Functional direction as an arrow
pub fn render_stackup_svg(
    stackup: &Stackup,
    contributors_3d: &[ChainContributor3D],
    config: &SvgConfig,
) -> String {
    let mut svg = String::new();

    // SVG header with embedded styles
    svg.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
<style>
  .feature-box {{ fill: #4a90d9; stroke: #2c5282; stroke-width: 2; }}
  .tol-zone {{ fill: rgba(255, 220, 100, 0.3); stroke: #b7791f; stroke-width: 1; stroke-dasharray: 4,2; }}
  .chain-line {{ stroke: #4a5568; stroke-width: 2; marker-end: url(#arrow); }}
  .axis-line {{ stroke: #718096; stroke-width: 1; }}
  .func-arrow {{ stroke: #38a169; stroke-width: 3; marker-end: url(#func-arrow); }}
  .label {{ font-family: monospace; font-size: 12px; fill: #1a202c; }}
  .title {{ font-family: sans-serif; font-size: 16px; font-weight: bold; fill: #1a202c; }}
</style>
<defs>
  <marker id="arrow" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
    <polygon points="0 0, 10 3.5, 0 7" fill="#4a5568" />
  </marker>
  <marker id="func-arrow" markerWidth="12" markerHeight="8" refX="11" refY="4" orient="auto">
    <polygon points="0 0, 12 4, 0 8" fill="#38a169" />
  </marker>
</defs>
"##,
        config.width, config.height, config.width, config.height
    ));

    // Title
    svg.push_str(&format!(
        r#"<text x="{}" y="30" class="title">{}</text>
"#,
        config.padding,
        escape_xml(&stackup.title)
    ));

    // Draw coordinate axes at origin
    let (ox, oy) = project_isometric(0.0, 0.0, 0.0, config);
    let (ax, ay) = project_isometric(20.0, 0.0, 0.0, config);
    let (bx, by) = project_isometric(0.0, 20.0, 0.0, config);
    let (cx, cy) = project_isometric(0.0, 0.0, 20.0, config);

    svg.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" class="axis-line" />
<text x="{:.1}" y="{:.1}" class="label">X</text>
<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" class="axis-line" />
<text x="{:.1}" y="{:.1}" class="label">Y</text>
<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" class="axis-line" />
<text x="{:.1}" y="{:.1}" class="label">Z</text>
"#,
        ox,
        oy,
        ax,
        ay,
        ax + 5.0,
        ay, // X axis
        ox,
        oy,
        bx,
        by,
        bx + 5.0,
        by, // Y axis
        ox,
        oy,
        cx,
        cy,
        cx + 5.0,
        cy - 5.0 // Z axis
    ));

    // Draw each contributor
    let box_size = 10.0; // mm
    let mut prev_center: Option<(f64, f64)> = None;

    for (i, contrib) in contributors_3d.iter().enumerate() {
        let [px, py, pz] = contrib.position;

        // Project box corners (simplified - just show center and bounding box)
        let (cx, cy) = project_isometric(px, py, pz, config);

        // Draw tolerance zone (slightly larger dashed box)
        let tol_size = box_size + 2.0;
        svg.push_str(&format!(
            r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" class="tol-zone" />
"#,
            cx - tol_size * config.scale / 2.0,
            cy - tol_size * config.scale / 2.0,
            tol_size * config.scale,
            tol_size * config.scale
        ));

        // Draw feature box
        svg.push_str(&format!(
            r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" class="feature-box" rx="3" />
"#,
            cx - box_size * config.scale / 2.0,
            cy - box_size * config.scale / 2.0,
            box_size * config.scale,
            box_size * config.scale
        ));

        // Label
        let label = if contrib.name.len() > 8 {
            format!("{}…", &contrib.name[..7])
        } else {
            contrib.name.clone()
        };
        svg.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" class="label" text-anchor="middle">{}</text>
"#,
            cx,
            cy + box_size * config.scale / 2.0 + 15.0,
            escape_xml(&label)
        ));

        // Chain connection line from previous
        if let Some((prev_x, prev_y)) = prev_center {
            svg.push_str(&format!(
                r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" class="chain-line" />
"#,
                prev_x, prev_y, cx, cy
            ));
        }

        prev_center = Some((cx, cy));

        // Position coordinates (small text)
        svg.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" class="label" font-size="10" opacity="0.7">[{:.0},{:.0},{:.0}]</text>
"#,
            cx,
            cy - box_size * config.scale / 2.0 - 5.0,
            px, py, pz
        ));

        // If it's the last contributor and we have functional direction, draw it
        if i == contributors_3d.len() - 1 {
            if let Some(dir) = stackup.functional_direction {
                let [dx, dy, dz] = dir;
                let arrow_len = 30.0;
                let (end_x, end_y) = project_isometric(
                    px + dx * arrow_len,
                    py + dy * arrow_len,
                    pz + dz * arrow_len,
                    config,
                );
                svg.push_str(&format!(
                    r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" class="func-arrow" />
<text x="{:.1}" y="{:.1}" class="label" fill="#38a169">Func Dir</text>
"##,
                    cx,
                    cy,
                    end_x,
                    end_y,
                    end_x + 10.0,
                    end_y
                ));
            }
        }
    }

    // Legend
    let legend_x = config.width as f64 - 150.0;
    let legend_y = config.height as f64 - 100.0;
    svg.push_str(&format!(
        r#"<g transform="translate({:.0},{:.0})">
  <rect x="0" y="0" width="15" height="15" class="feature-box" rx="2" />
  <text x="20" y="12" class="label">Feature</text>
  <rect x="0" y="25" width="15" height="15" class="tol-zone" />
  <text x="20" y="37" class="label">Tolerance Zone</text>
  <line x1="0" y1="55" x2="15" y2="55" class="chain-line" />
  <text x="20" y="58" class="label">Chain</text>
</g>
"#,
        legend_x, legend_y
    ));

    // Close SVG
    svg.push_str("</svg>\n");

    svg
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ============================================================================
// ASCII Isometric Visualization
// ============================================================================

/// Render a 3D stackup as ASCII isometric visualization
///
/// Uses braille characters for pseudo-3D rendering
pub fn render_isometric_ascii(stackup: &Stackup, contributors_3d: &[ChainContributor3D]) -> String {
    const CANVAS_WIDTH: u32 = 120;
    const CANVAS_HEIGHT: u32 = 50;

    if contributors_3d.is_empty() {
        return "  (no 3D contributors)".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("\n3D Tolerance Stack: {}\n", stackup.title));
    output.push_str("═".repeat(70).as_str());
    output.push('\n');

    // ===== Braille 3D Visualization =====
    let mut canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);

    // Calculate bounding box of all contributors
    let (min_bounds, max_bounds) = calculate_3d_bounds(contributors_3d);

    // Calculate scale to fit in canvas
    let range_x = (max_bounds[0] - min_bounds[0]).max(1.0);
    let range_y = (max_bounds[1] - min_bounds[1]).max(1.0);
    let range_z = (max_bounds[2] - min_bounds[2]).max(1.0);

    let scale = (CANVAS_WIDTH as f64 * 0.25) / range_x.max(range_y).max(range_z);

    // Center offset
    let center_x = (min_bounds[0] + max_bounds[0]) / 2.0;
    let center_y = (min_bounds[1] + max_bounds[1]) / 2.0;
    let center_z = (min_bounds[2] + max_bounds[2]) / 2.0;

    // Isometric projection helper
    let project = |x: f64, y: f64, z: f64| -> (u32, u32) {
        let nx = x - center_x;
        let ny = y - center_y;
        let nz = z - center_z;

        // Isometric projection (30 degree angles)
        let iso_x = (nx - ny) * 0.866 * scale; // cos(30)
        let iso_y = (-nz + (nx + ny) * 0.5) * scale; // sin(30)

        let px = (CANVAS_WIDTH as f64 / 2.0 + iso_x) as u32;
        let py = (CANVAS_HEIGHT as f64 / 2.0 + iso_y) as u32;

        (px.min(CANVAS_WIDTH - 1), py.min(CANVAS_HEIGHT - 1))
    };

    // Draw coordinate axes
    let (ox, oy) = project(center_x, center_y, center_z);
    let axis_len = 20.0;

    // X axis
    let (ax, ay) = project(center_x + axis_len, center_y, center_z);
    draw_line(&mut canvas, ox, oy, ax, ay);

    // Y axis
    let (bx, by) = project(center_x, center_y + axis_len, center_z);
    draw_line(&mut canvas, ox, oy, bx, by);

    // Z axis (up)
    let (zx, zy) = project(center_x, center_y, center_z + axis_len);
    draw_line(&mut canvas, ox, oy, zx, zy);

    // Draw each contributor with geometry-specific shapes
    let mut prev_pos: Option<(u32, u32)> = None;

    for contrib in contributors_3d.iter() {
        let [px, py, pz] = contrib.position;
        let (cx, cy) = project(px, py, pz);

        // Draw geometry-specific shape based on geometry class
        if cx > 5 && cx < CANVAS_WIDTH - 5 && cy > 5 && cy < CANVAS_HEIGHT - 5 {
            let shape_size = 4u32;
            draw_geometry_shape(
                &mut canvas,
                cx,
                cy,
                contrib.geometry_class,
                contrib.axis,
                shape_size,
            );
        }

        // Chain connection line (dashed effect by drawing every other segment)
        if let Some((prev_x, prev_y)) = prev_pos {
            draw_line(&mut canvas, prev_x, prev_y, cx, cy);
        }
        prev_pos = Some((cx, cy));
    }

    // Draw functional direction arrow
    if let Some(dir) = stackup.functional_direction {
        if let Some((last_x, last_y)) = prev_pos {
            let arrow_len = 15.0;
            let [dx, dy, dz] = dir;
            let last_contrib = contributors_3d.last().unwrap();
            let [px, py, pz] = last_contrib.position;
            let (end_x, end_y) = project(
                px + dx * arrow_len,
                py + dy * arrow_len,
                pz + dz * arrow_len,
            );
            draw_line(&mut canvas, last_x, last_y, end_x, end_y);
        }
    }

    output.push_str(&canvas.frame());
    output.push_str(
        "  X→  Y↗  Z↑   Legend: ▭=Plane  ○=Cylinder  ●=Sphere  △=Cone  +=Point  ◇=Complex\n",
    );

    // ===== Informative Text Section =====
    output.push_str("\n┌─ Contributors ────────────────────────────────────────────────────┐\n");

    for (i, (contrib_3d, contrib)) in contributors_3d
        .iter()
        .zip(stackup.contributors.iter())
        .enumerate()
    {
        let dir_symbol = match contrib.direction {
            tdt_core::entities::stackup::Direction::Positive => "→ +",
            tdt_core::entities::stackup::Direction::Negative => "← −",
        };

        // Build torsor bounds display
        let bounds_w = if let Some(w) = contrib_3d.bounds.w {
            format!("w:[{:+.3},{:+.3}]", w[0], w[1])
        } else {
            "w:[0]".to_string()
        };

        let bounds_alpha = if let Some(a) = contrib_3d.bounds.alpha {
            format!("α:[{:+.4},{:+.4}]", a[0], a[1])
        } else {
            "".to_string()
        };

        output.push_str(&format!(
            "│ {:>2}. {} {:18} {:>8.3} ±{:.3}/{:.3} mm            │\n",
            i + 1,
            dir_symbol,
            truncate_str(&contrib_3d.name, 18),
            contrib.nominal,
            contrib.plus_tol,
            contrib.minus_tol,
        ));

        output.push_str(&format!(
            "│     {:10} @ [{:>5.1},{:>5.1},{:>5.1}]  {} {}  │\n",
            format!("{}", contrib_3d.geometry_class),
            contrib_3d.position[0],
            contrib_3d.position[1],
            contrib_3d.position[2],
            bounds_w,
            bounds_alpha,
        ));

        if i < contributors_3d.len() - 1 {
            output.push_str(
                "│     ↓                                                             │\n",
            );
        }
    }

    output.push_str("└───────────────────────────────────────────────────────────────────┘\n");

    // Chain calculation
    output.push_str("\nStack Calculation:\n  ");
    let mut running_total: f64 = 0.0;

    for (i, contrib) in stackup.contributors.iter().enumerate() {
        let sign = match contrib.direction {
            tdt_core::entities::stackup::Direction::Positive => "+",
            tdt_core::entities::stackup::Direction::Negative => "-",
        };
        running_total += match contrib.direction {
            tdt_core::entities::stackup::Direction::Positive => contrib.nominal,
            tdt_core::entities::stackup::Direction::Negative => -contrib.nominal,
        };

        if i > 0 {
            output.push(' ');
        }
        output.push_str(&format!("{}{:.1}", sign, contrib.nominal));
    }
    output.push_str(&format!(" = {:.3} mm\n", running_total));

    // Target comparison
    output.push_str(&format!(
        "\nTarget: {} = {:.3} mm  [LSL: {:.3}, USL: {:.3}]\n",
        stackup.target.name,
        stackup.target.nominal,
        stackup.target.lower_limit,
        stackup.target.upper_limit
    ));

    if let Some(dir) = stackup.functional_direction {
        output.push_str(&format!(
            "Functional Direction: [{:.1}, {:.1}, {:.1}]\n",
            dir[0], dir[1], dir[2]
        ));
    }

    output
}

/// Calculate bounding box of 3D contributors
fn calculate_3d_bounds(contributors: &[ChainContributor3D]) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::MAX, f64::MAX, f64::MAX];
    let mut max = [f64::MIN, f64::MIN, f64::MIN];

    for contrib in contributors {
        for i in 0..3 {
            min[i] = min[i].min(contrib.position[i]);
            max[i] = max[i].max(contrib.position[i]);
        }
    }

    // Add some padding
    let padding = 10.0;
    for i in 0..3 {
        min[i] -= padding;
        max[i] += padding;
    }

    (min, max)
}

/// Draw a line using Bresenham's algorithm
fn draw_line(canvas: &mut Canvas, x0: u32, y0: u32, x1: u32, y1: u32) {
    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx = if x0 < x1 { 1i32 } else { -1i32 };
    let sy = if y0 < y1 { 1i32 } else { -1i32 };
    let mut err = dx + dy;

    let mut x = x0 as i32;
    let mut y = y0 as i32;

    loop {
        if x >= 0 && y >= 0 {
            canvas.set(x as u32, y as u32);
        }

        if x == x1 as i32 && y == y1 as i32 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 as i32 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 as i32 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
}

/// Draw an ellipse (for cylinder/sphere shapes)
fn draw_ellipse(canvas: &mut Canvas, cx: u32, cy: u32, rx: u32, ry: u32) {
    let steps = 32;
    for i in 0..steps {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / (steps as f64);
        let px = cx as f64 + rx as f64 * theta.cos();
        let py = cy as f64 + ry as f64 * theta.sin();
        if px >= 0.0 && py >= 0.0 {
            canvas.set(px as u32, py as u32);
        }
    }
}

/// Draw a filled circle (for sphere)
fn draw_filled_circle(canvas: &mut Canvas, cx: u32, cy: u32, r: u32) {
    for y in 0..=r * 2 {
        for x in 0..=r * 2 {
            let dx = x as i32 - r as i32;
            let dy = y as i32 - r as i32;
            if dx * dx + dy * dy <= (r * r) as i32 {
                let px = cx as i32 + dx;
                let py = cy as i32 + dy;
                if px >= 0 && py >= 0 {
                    canvas.set(px as u32, py as u32);
                }
            }
        }
    }
}

/// Draw a rectangle (for plane shapes)
fn draw_rect(canvas: &mut Canvas, cx: u32, cy: u32, w: u32, h: u32) {
    let x0 = cx.saturating_sub(w / 2);
    let y0 = cy.saturating_sub(h / 2);
    let x1 = cx + w / 2;
    let y1 = cy + h / 2;

    // Top and bottom edges
    for x in x0..=x1 {
        canvas.set(x, y0);
        canvas.set(x, y1);
    }
    // Left and right edges
    for y in y0..=y1 {
        canvas.set(x0, y);
        canvas.set(x1, y);
    }
}

/// Draw a cone/triangle shape
fn draw_triangle(canvas: &mut Canvas, cx: u32, cy: u32, size: u32) {
    let top_y = cy.saturating_sub(size);
    let base_y = cy + size / 2;
    let left_x = cx.saturating_sub(size);
    let right_x = cx + size;

    // Draw three sides
    draw_line(canvas, cx, top_y, left_x, base_y);
    draw_line(canvas, cx, top_y, right_x, base_y);
    draw_line(canvas, left_x, base_y, right_x, base_y);
}

/// Draw a point/cross marker
fn draw_cross(canvas: &mut Canvas, cx: u32, cy: u32, size: u32) {
    // Horizontal line
    for x in cx.saturating_sub(size)..=cx + size {
        canvas.set(x, cy);
    }
    // Vertical line
    for y in cy.saturating_sub(size)..=cy + size {
        canvas.set(cx, y);
    }
}

/// Draw a short line segment (for line geometry)
fn draw_line_segment(canvas: &mut Canvas, cx: u32, cy: u32, size: u32, axis: [f64; 3]) {
    // Draw a line in the direction of the axis
    let dx = (axis[0] * size as f64) as i32;
    let dy = (axis[2] * size as f64) as i32; // Z maps to vertical

    let x0 = (cx as i32 - dx).max(0) as u32;
    let y0 = (cy as i32 + dy).max(0) as u32;
    let x1 = (cx as i32 + dx).max(0) as u32;
    let y1 = (cy as i32 - dy).max(0) as u32;

    draw_line(canvas, x0, y0, x1, y1);
    // Add endpoints
    canvas.set(x0, y0);
    canvas.set(x1, y1);
}

/// Draw a diamond shape (for complex geometry)
fn draw_diamond(canvas: &mut Canvas, cx: u32, cy: u32, size: u32) {
    draw_line(
        canvas,
        cx.saturating_sub(size),
        cy,
        cx,
        cy.saturating_sub(size),
    );
    draw_line(canvas, cx, cy.saturating_sub(size), cx + size, cy);
    draw_line(canvas, cx + size, cy, cx, cy + size);
    draw_line(canvas, cx, cy + size, cx.saturating_sub(size), cy);
}

use tdt_core::entities::feature::GeometryClass;

/// Draw geometry-specific shape based on geometry class
fn draw_geometry_shape(
    canvas: &mut Canvas,
    cx: u32,
    cy: u32,
    geometry_class: GeometryClass,
    axis: [f64; 3],
    size: u32,
) {
    match geometry_class {
        GeometryClass::Plane => {
            // Draw a flat rectangle, oriented based on axis
            if axis[2].abs() > 0.5 {
                // Horizontal plane (XY)
                draw_rect(canvas, cx, cy, size * 2, size);
            } else if axis[0].abs() > 0.5 {
                // YZ plane
                draw_rect(canvas, cx, cy, size, size * 2);
            } else {
                // XZ plane
                draw_rect(canvas, cx, cy, size * 2, size);
            }
        }
        GeometryClass::Cylinder => {
            // Draw an ellipse (circle viewed from angle)
            if axis[2].abs() > 0.5 {
                // Cylinder along Z - shows as circle
                draw_ellipse(canvas, cx, cy, size, size);
            } else {
                // Cylinder along X or Y - shows as ellipse
                draw_ellipse(canvas, cx, cy, size * 2, size);
            }
        }
        GeometryClass::Sphere => {
            // Draw a filled circle
            draw_filled_circle(canvas, cx, cy, size);
        }
        GeometryClass::Cone => {
            // Draw a triangle
            draw_triangle(canvas, cx, cy, size);
        }
        GeometryClass::Point => {
            // Draw a cross marker
            draw_cross(canvas, cx, cy, size / 2);
        }
        GeometryClass::Line => {
            // Draw a line segment in the direction of the axis
            draw_line_segment(canvas, cx, cy, size, axis);
        }
        GeometryClass::Complex => {
            // Draw a diamond (original shape)
            draw_diamond(canvas, cx, cy, size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdt_core::entities::stackup::{Contributor, Direction, Distribution, Target, TorsorStats};

    #[allow(clippy::field_reassign_with_default)] // Stackup has too many fields for a literal
    fn make_test_stackup() -> Stackup {
        let mut stackup = Stackup::default();
        // Use a new ID instead of parsing
        stackup.title = "Test Gap Stackup".to_string();
        stackup.target = Target {
            name: "Gap".to_string(),
            nominal: 1.0,
            upper_limit: 1.5,
            lower_limit: 0.5,
            units: "mm".to_string(),
            critical: false,
        };

        // Add contributors
        stackup.contributors.push(Contributor {
            name: "Housing Length".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 100.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        stackup.contributors.push(Contributor {
            name: "Shaft Length".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 99.0,
            plus_tol: 0.05,
            minus_tol: 0.05,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        stackup
    }

    #[test]
    fn test_render_chain_schematic_basic() {
        let stackup = make_test_stackup();
        let output = render_chain_schematic(&stackup);

        // Debug: print the actual output
        println!("Chain schematic output:\n{}", output);

        // Check that output contains expected elements
        assert!(output.contains("Test Gap Stackup"), "Should contain title");
        // The names get truncated to 6 chars
        assert!(
            output.contains("Housi") || output.contains("Housin"),
            "Should contain truncated first contributor"
        );
        assert!(
            output.contains("Shaft"),
            "Should contain truncated second contributor"
        );
        assert!(output.contains("→"), "Should contain direction arrows");
    }

    #[test]
    fn test_render_chain_schematic_empty() {
        let stackup = Stackup {
            title: "Empty".to_string(),
            ..Stackup::default()
        };

        let output = render_chain_schematic(&stackup);
        assert!(output.contains("no contributors"));
    }

    #[test]
    fn test_render_deviation_ellipse() {
        let result = ResultTorsor {
            u: TorsorStats {
                wc_min: -0.1,
                wc_max: 0.1,
                rss_mean: 0.0,
                rss_3sigma: 0.08,
                mc_mean: None,
                mc_std_dev: None,
            },
            v: TorsorStats {
                wc_min: -0.05,
                wc_max: 0.05,
                rss_mean: 0.0,
                rss_3sigma: 0.04,
                mc_mean: None,
                mc_std_dev: None,
            },
            w: TorsorStats::default(),
            alpha: TorsorStats::default(),
            beta: TorsorStats::default(),
            gamma: TorsorStats::default(),
        };

        let output = render_deviation_ellipse(&result, 32);

        // Check that output contains expected elements
        assert!(output.contains("UV Deviation (3σ)"));
        assert!(output.contains("U:"));
        assert!(output.contains("V:"));
        // Should contain braille characters
        assert!(output
            .chars()
            .any(|c| c as u32 >= 0x2800 && c as u32 <= 0x28FF));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("verylongstring", 6), "veryl…");
        assert_eq!(truncate_str("ab", 2), "ab");
        assert_eq!(truncate_str("abc", 2), "ab");
    }
}
