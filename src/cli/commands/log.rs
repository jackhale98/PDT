//! `tdt log` command - View workflow activity across the project
//!
//! Shows a chronological log of workflow events (approvals, releases) across all entities.

use chrono::{DateTime, Utc};
use console::style;
use miette::Result;
use serde::Deserialize;
use std::collections::HashMap;
use walkdir::WalkDir;

use crate::core::identity::EntityPrefix;
use crate::core::project::Project;
use crate::core::workflow::{truncate_id, ApprovalRecord};

#[derive(clap::Args, Debug)]
pub struct LogArgs {
    /// Filter by approver name
    #[arg(long, short = 'a')]
    pub approver: Option<String>,

    /// Filter by entity type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Filter by event type (approval, release)
    #[arg(long, short = 'e')]
    pub event_type: Option<String>,

    /// Show events since date (YYYY-MM-DD)
    #[arg(long)]
    pub since: Option<String>,

    /// Show events until date (YYYY-MM-DD)
    #[arg(long)]
    pub until: Option<String>,

    /// Limit number of events
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Output format (table, json)
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,
}

/// Workflow event for log output
#[derive(Debug, Clone, serde::Serialize)]
struct LogEntry {
    timestamp: DateTime<Utc>,
    event_type: String,
    entity_id: String,
    entity_type: String,
    entity_title: String,
    actor: String,
    role: Option<String>,
    comment: Option<String>,
}

/// Entity data for extracting workflow events
#[derive(Debug, Deserialize)]
struct EntityData {
    id: String,
    title: String,
    #[serde(default)]
    approvals: Vec<ApprovalRecord>,
    #[serde(default)]
    released_by: Option<String>,
    #[serde(default)]
    released_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    _extra: HashMap<String, serde_yml::Value>,
}

pub fn run(args: LogArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Parse date filters
    let since_date = args.since.as_ref().and_then(|s| parse_date(s));
    let until_date = args.until.as_ref().and_then(|s| parse_date(s));

    // Parse entity type filter
    let entity_type_filter: Option<EntityPrefix> = args
        .entity_type
        .as_ref()
        .and_then(|t| t.to_uppercase().parse().ok());

    // Collect all workflow events
    let mut entries: Vec<LogEntry> = Vec::new();

    for entry in WalkDir::new(project.root())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml")
                .unwrap_or(false)
        })
        .filter(|e| e.path().to_string_lossy().contains(".tdt.yaml"))
    {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            if let Ok(entity) = serde_yml::from_str::<EntityData>(&content) {
                let entity_prefix = get_prefix_from_id(&entity.id);

                // Filter by entity type
                if let Some(ref filter) = entity_type_filter {
                    if entity_prefix.as_ref() != Some(filter) {
                        continue;
                    }
                }

                let entity_type_str = entity_prefix
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_else(|| "???".to_string());

                // Extract approval events
                for approval in &entity.approvals {
                    // Filter by approver
                    if let Some(ref approver_filter) = args.approver {
                        if !approval
                            .approver
                            .to_lowercase()
                            .contains(&approver_filter.to_lowercase())
                        {
                            continue;
                        }
                    }

                    // Filter by event type
                    if let Some(ref event_filter) = args.event_type {
                        if event_filter.to_lowercase() != "approval" {
                            continue;
                        }
                    }

                    // Filter by date range
                    if let Some(since) = since_date {
                        if approval.timestamp < since {
                            continue;
                        }
                    }
                    if let Some(until) = until_date {
                        if approval.timestamp > until {
                            continue;
                        }
                    }

                    entries.push(LogEntry {
                        timestamp: approval.timestamp,
                        event_type: "APPROVE".to_string(),
                        entity_id: truncate_id(&entity.id),
                        entity_type: entity_type_str.clone(),
                        entity_title: entity.title.clone(),
                        actor: approval.approver.clone(),
                        role: approval.role.clone(),
                        comment: approval.comment.clone(),
                    });
                }

                // Extract release event
                if let (Some(releaser), Some(released_at)) =
                    (&entity.released_by, entity.released_at)
                {
                    // Filter by approver (also applies to releaser)
                    if let Some(ref approver_filter) = args.approver {
                        if !releaser
                            .to_lowercase()
                            .contains(&approver_filter.to_lowercase())
                        {
                            continue;
                        }
                    }

                    // Filter by event type
                    if let Some(ref event_filter) = args.event_type {
                        if event_filter.to_lowercase() != "release" {
                            continue;
                        }
                    }

                    // Filter by date range
                    if let Some(since) = since_date {
                        if released_at < since {
                            continue;
                        }
                    }
                    if let Some(until) = until_date {
                        if released_at > until {
                            continue;
                        }
                    }

                    entries.push(LogEntry {
                        timestamp: released_at,
                        event_type: "RELEASE".to_string(),
                        entity_id: truncate_id(&entity.id),
                        entity_type: entity_type_str.clone(),
                        entity_title: entity.title.clone(),
                        actor: releaser.clone(),
                        role: Some("management".to_string()),
                        comment: None,
                    });
                }
            }
        }
    }

    // Sort by timestamp (newest first)
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Apply limit
    if let Some(limit) = args.limit {
        entries.truncate(limit);
    }

    // Output
    match args.output.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&entries)
                .map_err(|e| miette::miette!("JSON error: {}", e))?;
            println!("{}", json);
        }
        _ => {
            if entries.is_empty() {
                println!("No workflow events found.");
                return Ok(());
            }

            println!(
                "\n{}\n",
                style("Workflow Activity Log").bold().underlined()
            );
            println!(
                "{:<12} {:<10} {:<8} {:<15} {:<15} COMMENT",
                "DATE",
                "EVENT",
                "TYPE",
                "ENTITY",
                "ACTOR"
            );
            println!("{}", "-".repeat(80));

            for entry in &entries {
                let date_str = entry.timestamp.format("%Y-%m-%d");
                let event_styled = match entry.event_type.as_str() {
                    "APPROVE" => style(&entry.event_type).green(),
                    "RELEASE" => style(&entry.event_type).magenta(),
                    _ => style(&entry.event_type).white(),
                };
                let actor_with_role = if let Some(ref role) = entry.role {
                    format!("{} ({})", entry.actor, role)
                } else {
                    entry.actor.clone()
                };
                let comment = entry
                    .comment
                    .as_ref()
                    .map(|c| truncate_str(c, 20))
                    .unwrap_or_default();

                println!(
                    "{:<12} {:<10} {:<8} {:<15} {:<15} {}",
                    style(date_str).dim(),
                    event_styled,
                    entry.entity_type,
                    style(&entry.entity_id).cyan(),
                    actor_with_role,
                    style(comment).dim()
                );
            }

            println!("\n{} workflow events.", entries.len());
        }
    }

    Ok(())
}

fn get_prefix_from_id(id: &str) -> Option<EntityPrefix> {
    id.split('-').next().and_then(|s| s.parse().ok())
}

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    // Try parsing as YYYY-MM-DD
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
