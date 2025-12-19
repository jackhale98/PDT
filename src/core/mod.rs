//! Core module - fundamental types and utilities

pub mod cache;
pub mod config;
pub mod entity;
pub mod identity;
pub mod links;
pub mod loader;
pub mod project;
pub mod shortid;

pub use cache::{
    CachedComponent, CachedEntity, CachedFeature, CachedLink, CachedQuote, CachedRequirement,
    CachedRisk, CachedSupplier, CachedTest, EntityCache, EntityFilter, LinkType, SyncStats,
};
pub use config::Config;
pub use entity::Entity;
pub use identity::{EntityId, EntityPrefix, IdParseError};
pub use project::{Project, ProjectError};
pub use shortid::ShortIdIndex;
