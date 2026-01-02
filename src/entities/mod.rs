//! Entity type definitions
//!
//! TDT supports the following entity types:
//!
//! **Product Development:**
//! - [`Requirement`] - Input/output requirements with traceability
//! - [`Hazard`] - Potential sources of harm (safety analysis)
//! - [`Risk`] - Design and process risks with FMEA analysis
//! - [`Test`] - Verification and validation test protocols
//! - [`Result`] - Test execution results and verdicts
//!
//! **BOM Management:**
//! - [`Component`] - Individual parts (make/buy) with suppliers
//! - [`Assembly`] - Collections of components with BOM quantities
//! - [`Supplier`] - Approved suppliers with contact info and certifications
//! - [`Quote`] - Supplier quotations with pricing and lead times
//!
//! **Tolerance Analysis:**
//! - [`Feature`] - Dimensional features on components with tolerances
//! - [`Mate`] - 1:1 contact between features with fit calculation
//! - [`Stackup`] - Tolerance chain analysis with worst-case, RSS, and Monte Carlo

pub mod assembly;
pub mod capa;
pub mod component;
pub mod control;
pub mod dev;
pub mod feature;
pub mod hazard;
pub mod lot;
pub mod mate;
pub mod ncr;
pub mod process;
pub mod quote;
pub mod requirement;
pub mod result;
pub mod risk;
pub mod safety;
pub mod stackup;
pub mod supplier;
pub mod test;
pub mod work_instruction;

pub use assembly::Assembly;
pub use capa::Capa;
pub use component::{Component, ComponentSupplier};
pub use control::Control;
pub use dev::Dev;
pub use feature::Feature;
pub use hazard::Hazard;
pub use lot::Lot;
pub use mate::Mate;
pub use ncr::Ncr;
pub use process::Process;
pub use quote::Quote;
pub use requirement::Requirement;
pub use result::Result;
pub use risk::Risk;
pub use safety::{Asil, Dal, SwClass};
pub use stackup::Stackup;
pub use supplier::Supplier;
pub use test::Test;
pub use work_instruction::WorkInstruction;
