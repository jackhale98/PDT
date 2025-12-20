//! Unified filter enums for CLI commands
//!
//! This module consolidates filter types used across entity commands,
//! eliminating duplication and ensuring consistent behavior.

use clap::ValueEnum;

use crate::core::entity::{Priority, Status};

/// Status filter for list commands
///
/// Used to filter entities by their workflow status.
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum StatusFilter {
    /// Draft status only
    Draft,
    /// Review status only
    Review,
    /// Approved status only
    Approved,
    /// Released status only
    Released,
    /// Obsolete status only
    Obsolete,
    /// All active statuses (not obsolete) - default
    #[default]
    Active,
    /// All statuses including obsolete
    All,
}

impl StatusFilter {
    /// Check if a Status matches this filter
    pub fn matches(&self, status: &Status) -> bool {
        match self {
            StatusFilter::Draft => *status == Status::Draft,
            StatusFilter::Review => *status == Status::Review,
            StatusFilter::Approved => *status == Status::Approved,
            StatusFilter::Released => *status == Status::Released,
            StatusFilter::Obsolete => *status == Status::Obsolete,
            StatusFilter::Active => *status != Status::Obsolete,
            StatusFilter::All => true,
        }
    }

    /// Check if a status string matches this filter
    ///
    /// Used during migration from string-based status fields.
    pub fn matches_str(&self, status: &str) -> bool {
        match self {
            StatusFilter::Draft => status == "draft",
            StatusFilter::Review => status == "review",
            StatusFilter::Approved => status == "approved",
            StatusFilter::Released => status == "released",
            StatusFilter::Obsolete => status == "obsolete",
            StatusFilter::Active => status != "obsolete",
            StatusFilter::All => true,
        }
    }
}

impl std::fmt::Display for StatusFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusFilter::Draft => write!(f, "draft"),
            StatusFilter::Review => write!(f, "review"),
            StatusFilter::Approved => write!(f, "approved"),
            StatusFilter::Released => write!(f, "released"),
            StatusFilter::Obsolete => write!(f, "obsolete"),
            StatusFilter::Active => write!(f, "active"),
            StatusFilter::All => write!(f, "all"),
        }
    }
}

/// Priority filter for list commands
///
/// Used to filter entities by their priority level.
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum PriorityFilter {
    /// Low priority only
    Low,
    /// Medium priority only
    Medium,
    /// High priority only
    High,
    /// Critical priority only
    Critical,
    /// High and critical only
    Urgent,
    /// All priorities - default
    #[default]
    All,
}

impl PriorityFilter {
    /// Check if a Priority matches this filter
    pub fn matches(&self, priority: &Priority) -> bool {
        match self {
            PriorityFilter::Low => *priority == Priority::Low,
            PriorityFilter::Medium => *priority == Priority::Medium,
            PriorityFilter::High => *priority == Priority::High,
            PriorityFilter::Critical => *priority == Priority::Critical,
            PriorityFilter::Urgent => {
                *priority == Priority::High || *priority == Priority::Critical
            }
            PriorityFilter::All => true,
        }
    }

    /// Check if a priority string matches this filter
    ///
    /// Used during migration from string-based priority fields.
    pub fn matches_str(&self, priority: &str) -> bool {
        match self {
            PriorityFilter::Low => priority == "low",
            PriorityFilter::Medium => priority == "medium",
            PriorityFilter::High => priority == "high",
            PriorityFilter::Critical => priority == "critical",
            PriorityFilter::Urgent => priority == "high" || priority == "critical",
            PriorityFilter::All => true,
        }
    }

    /// Check if an optional priority string matches this filter
    ///
    /// Returns true if priority is None and filter is All.
    pub fn matches_option_str(&self, priority: Option<&str>) -> bool {
        match priority {
            Some(p) => self.matches_str(p),
            None => *self == PriorityFilter::All,
        }
    }
}

impl std::fmt::Display for PriorityFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriorityFilter::Low => write!(f, "low"),
            PriorityFilter::Medium => write!(f, "medium"),
            PriorityFilter::High => write!(f, "high"),
            PriorityFilter::Critical => write!(f, "critical"),
            PriorityFilter::Urgent => write!(f, "urgent"),
            PriorityFilter::All => write!(f, "all"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_filter_matches() {
        assert!(StatusFilter::Draft.matches(&Status::Draft));
        assert!(!StatusFilter::Draft.matches(&Status::Review));

        assert!(StatusFilter::Active.matches(&Status::Draft));
        assert!(StatusFilter::Active.matches(&Status::Approved));
        assert!(!StatusFilter::Active.matches(&Status::Obsolete));

        assert!(StatusFilter::All.matches(&Status::Draft));
        assert!(StatusFilter::All.matches(&Status::Obsolete));
    }

    #[test]
    fn test_status_filter_matches_str() {
        assert!(StatusFilter::Draft.matches_str("draft"));
        assert!(!StatusFilter::Draft.matches_str("review"));

        assert!(StatusFilter::Active.matches_str("draft"));
        assert!(!StatusFilter::Active.matches_str("obsolete"));

        assert!(StatusFilter::All.matches_str("obsolete"));
    }

    #[test]
    fn test_priority_filter_matches() {
        assert!(PriorityFilter::High.matches(&Priority::High));
        assert!(!PriorityFilter::High.matches(&Priority::Low));

        assert!(PriorityFilter::Urgent.matches(&Priority::High));
        assert!(PriorityFilter::Urgent.matches(&Priority::Critical));
        assert!(!PriorityFilter::Urgent.matches(&Priority::Medium));

        assert!(PriorityFilter::All.matches(&Priority::Low));
    }

    #[test]
    fn test_priority_filter_matches_str() {
        assert!(PriorityFilter::High.matches_str("high"));
        assert!(!PriorityFilter::High.matches_str("low"));

        assert!(PriorityFilter::Urgent.matches_str("high"));
        assert!(PriorityFilter::Urgent.matches_str("critical"));
        assert!(!PriorityFilter::Urgent.matches_str("medium"));
    }

    #[test]
    fn test_priority_filter_option() {
        assert!(PriorityFilter::All.matches_option_str(None));
        assert!(!PriorityFilter::High.matches_option_str(None));
        assert!(PriorityFilter::High.matches_option_str(Some("high")));
    }
}
