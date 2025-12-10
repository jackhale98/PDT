//! Core module - fundamental types and utilities

pub mod config;
pub mod entity;
pub mod identity;
pub mod project;

pub use config::Config;
pub use entity::Entity;
pub use identity::{EntityId, EntityPrefix, IdParseError};
pub use project::{Project, ProjectError};
