//! Core module - fundamental types and utilities

pub mod cache;
pub mod config;
pub mod entity;
pub mod git;
pub mod identity;
pub mod links;
pub mod loader;
pub mod project;
pub mod provider;
pub mod shortid;
pub mod suspect;
pub mod team;
pub mod workflow;

pub use cache::{
    CachedComponent, CachedEntity, CachedFeature, CachedLink, CachedQuote, CachedRequirement,
    CachedRisk, CachedSupplier, CachedTest, EntityCache, EntityFilter, LinkType, SyncStats,
};
pub use config::Config;
pub use entity::Entity;
pub use git::{Git, GitError};
pub use identity::{EntityId, EntityPrefix, IdParseError};
pub use project::{Project, ProjectError};
pub use provider::{PrInfo, PrState, Provider, ProviderClient, ProviderError};
pub use shortid::ShortIdIndex;
pub use suspect::{
    clear_link_suspect, get_suspect_links, has_suspect_links, mark_link_suspect, ExtendedLinkRef,
    LinkRef, SuspectError, SuspectReason, SuspectSummary,
};
pub use team::{Role, TeamMember, TeamRoster};
pub use workflow::{WorkflowConfig, WorkflowEngine, WorkflowError};
