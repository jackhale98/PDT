//! Terminal visualization using braille graphics
//!
//! Provides terminal-based visualization for tolerance chains and analysis results
//! using Unicode braille characters for graphical rendering.

use drawille::Canvas;

use crate::entities::stackup::{ResultTorsor, Stackup};

/// Default canvas size for chain schematic
pub const CHAIN_WIDTH: u32 = 120;
pub const CHAIN_HEIGHT: u32 = 16;

/// Default canvas size for deviation ellipse
pub const ELLIPSE_SIZE: u32 = 32;

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
                crate::entities::stackup::Direction::Positive => "→",
                crate::entities::stackup::Direction::Negative => "←",
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

/// Render a simple 1D tolerance range bar
///
/// Shows min/max range with spec limits
pub fn render_range_bar(min: f64, max: f64, lower_limit: f64, upper_limit: f64) -> String {
    let bar_width = 60;

    // Calculate positions
    let full_range = upper_limit - lower_limit;
    let spec_margin = full_range * 0.1; // 10% margin outside spec

    let view_min = lower_limit - spec_margin;
    let view_max = upper_limit + spec_margin;
    let view_range = view_max - view_min;

    // Map values to bar positions
    let pos_lower = ((lower_limit - view_min) / view_range * bar_width as f64) as usize;
    let pos_upper = ((upper_limit - view_min) / view_range * bar_width as f64) as usize;
    let pos_min = ((min - view_min) / view_range * bar_width as f64) as usize;
    let pos_max = ((max - view_min) / view_range * bar_width as f64) as usize;

    let pos_min = pos_min.min(bar_width - 1).max(0);
    let pos_max = pos_max.min(bar_width - 1).max(0);
    let pos_lower = pos_lower.min(bar_width - 1).max(0);
    let pos_upper = pos_upper.min(bar_width - 1).max(0);

    // Build bar
    let mut bar: Vec<char> = vec!['─'; bar_width];

    // Mark spec limits
    bar[pos_lower] = '│';
    bar[pos_upper] = '│';

    // Mark result range
    for i in pos_min..=pos_max {
        if i < bar_width {
            bar[i] = if bar[i] == '│' { '╋' } else { '═' };
        }
    }

    // Mark min/max endpoints
    if pos_min < bar_width {
        bar[pos_min] = if bar[pos_min] == '│' { '╟' } else { '[' };
    }
    if pos_max < bar_width {
        bar[pos_max] = if bar[pos_max] == '│' { '╢' } else { ']' };
    }

    let bar_str: String = bar.into_iter().collect();

    format!(
        "  LSL={:.3}  USL={:.3}\n  {}\n  Min={:.4}  Max={:.4}",
        lower_limit, upper_limit, bar_str, min, max
    )
}

/// Render complete 3D analysis visualization
pub fn render_3d_analysis(stackup: &Stackup) -> String {
    let mut output = Vec::new();

    // Chain schematic
    output.push(render_chain_schematic(stackup));
    output.push(String::new());

    // 3D results if available
    if let Some(ref results_3d) = stackup.analysis_results_3d {
        if let Some(ref torsor) = results_3d.result_torsor {
            // Deviation ellipse
            output.push(render_deviation_ellipse(torsor, ELLIPSE_SIZE));
            output.push(String::new());

            // DOF summary table
            output.push("6-DOF Results (3σ):".to_string());
            output.push("  DOF    WC Min    WC Max    RSS Mean   RSS 3σ".to_string());
            output.push("  ─────  ────────  ────────  ─────────  ───────".to_string());
            output.push(format!(
                "  u      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.u.wc_min, torsor.u.wc_max, torsor.u.rss_mean, torsor.u.rss_3sigma
            ));
            output.push(format!(
                "  v      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.v.wc_min, torsor.v.wc_max, torsor.v.rss_mean, torsor.v.rss_3sigma
            ));
            output.push(format!(
                "  w      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.w.wc_min, torsor.w.wc_max, torsor.w.rss_mean, torsor.w.rss_3sigma
            ));
            output.push(format!(
                "  α      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.alpha.wc_min,
                torsor.alpha.wc_max,
                torsor.alpha.rss_mean,
                torsor.alpha.rss_3sigma
            ));
            output.push(format!(
                "  β      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.beta.wc_min,
                torsor.beta.wc_max,
                torsor.beta.rss_mean,
                torsor.beta.rss_3sigma
            ));
            output.push(format!(
                "  γ      {:>8.4}  {:>8.4}  {:>9.4}  {:>7.4}",
                torsor.gamma.wc_min,
                torsor.gamma.wc_max,
                torsor.gamma.rss_mean,
                torsor.gamma.rss_3sigma
            ));
        }
    }

    output.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::stackup::{Contributor, Direction, Distribution, Target, TorsorStats};

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
        let mut stackup = Stackup::default();
        stackup.title = "Empty".to_string();

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

        let output = render_deviation_ellipse(&result, ELLIPSE_SIZE);

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
    fn test_render_range_bar() {
        let output = render_range_bar(0.8, 1.2, 0.5, 1.5);

        // Check format
        assert!(output.contains("LSL=0.500"));
        assert!(output.contains("USL=1.500"));
        assert!(output.contains("Min=0.8000"));
        assert!(output.contains("Max=1.2000"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("verylongstring", 6), "veryl…");
        assert_eq!(truncate_str("ab", 2), "ab");
        assert_eq!(truncate_str("abc", 2), "ab");
    }

    #[test]
    fn test_render_3d_analysis_no_results() {
        let stackup = make_test_stackup();
        let output = render_3d_analysis(&stackup);

        // Should still render chain schematic
        assert!(output.contains("Test Gap Stackup"));
        // But no 3D results section since analysis_results_3d is None
        assert!(!output.contains("6-DOF Results"));
    }

    #[test]
    fn test_render_3d_analysis_with_results() {
        let mut stackup = make_test_stackup();
        stackup.analysis_results_3d = Some(crate::entities::stackup::Analysis3DResults {
            result_torsor: Some(ResultTorsor {
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
            }),
            sensitivity_3d: vec![],
            jacobian_summary: None,
            analyzed_at: None,
        });

        let output = render_3d_analysis(&stackup);

        assert!(output.contains("Test Gap Stackup"));
        assert!(output.contains("6-DOF Results"));
        assert!(output.contains("UV Deviation"));
    }
}
