//! CLI module - argument parsing and command dispatch

pub mod args;

/// Define a list-column enum for an entity's `list --columns` flag.
///
/// Generates the clap `ValueEnum` (with explicit value names), a `key()`
/// method returning the table-formatter column key, and a `Display` impl —
/// all guaranteed consistent, replacing 18 hand-written copies that had to
/// keep three representations in sync manually.
#[macro_export]
macro_rules! list_columns {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident => $key:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
        $vis enum $name {
            $( #[value(name = $key)] $variant, )+
        }

        impl $name {
            /// Column key used by the table formatter
            #[allow(dead_code)]
            pub const fn key(&self) -> &'static str {
                match self { $( Self::$variant => $key, )+ }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.key())
            }
        }
    };
}
pub mod commands;
pub mod entity_cmd;
pub mod filters;
pub mod helpers;
pub mod table;
pub mod viz;

pub use args::{Cli, Commands, GlobalOpts, OutputFormat};
pub use entity_cmd::EntityConfig;
