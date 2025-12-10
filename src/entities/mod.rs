//! Entity type definitions
//!
//! PDT supports the following entity types:
//!
//! **Product Development:**
//! - [`Requirement`] - Input/output requirements with traceability
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
pub mod component;
pub mod feature;
pub mod mate;
pub mod quote;
pub mod requirement;
pub mod result;
pub mod risk;
pub mod stackup;
pub mod supplier;
pub mod test;

pub use assembly::Assembly;
pub use component::Component;
pub use feature::Feature;
pub use mate::Mate;
pub use quote::Quote;
pub use requirement::Requirement;
pub use result::Result;
pub use risk::Risk;
pub use stackup::Stackup;
pub use supplier::Supplier;
pub use test::Test;
