//! CLI module - argument parsing and command dispatch

pub mod args;
pub mod commands;
pub mod output;

pub use args::{Cli, Commands, GlobalOpts, OutputFormat};
