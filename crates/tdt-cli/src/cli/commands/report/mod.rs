//! `tdt report` command - Generate engineering reports
//!
//! All helper functions use the EntityCache for fast lookups,
//! avoiding directory walks wherever possible.

mod bom;
mod fmea;
mod open_issues;
mod rvm;
mod test_status;
mod tolerance;

use clap::Subcommand;
use miette::{IntoDiagnostic, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::cli::GlobalOpts;
use tdt_core::core::cache::EntityCache;
use tdt_core::core::project::Project;
use tdt_core::entities::assembly::Assembly;
use tdt_core::entities::component::Component;
use tdt_core::entities::feature::Feature;
use tdt_core::entities::mate::Mate;
use tdt_core::entities::quote::Quote;
use tdt_core::entities::requirement::Requirement;
use tdt_core::entities::result::Result as TestResult;
use tdt_core::entities::risk::Risk;
use tdt_core::entities::stackup::Stackup;
use tdt_core::entities::test::Test;

pub use bom::BomArgs;
pub use fmea::FmeaArgs;
pub use open_issues::OpenIssuesArgs;
pub use rvm::RvmArgs;
pub use test_status::TestStatusArgs;
pub use tolerance::ToleranceArgs;

#[derive(Subcommand, Debug)]
pub enum ReportCommands {
    /// Requirements Verification Matrix (RVM)
    Rvm(RvmArgs),

    /// FMEA report sorted by RPN
    Fmea(FmeaArgs),

    /// BOM (Bill of Materials) with costs
    Bom(BomArgs),

    /// Test execution status summary
    TestStatus(TestStatusArgs),

    /// All open issues (NCRs, CAPAs, failed tests)
    OpenIssues(OpenIssuesArgs),

    /// Tolerance analysis report (features, mates, stackups by component)
    #[clap(alias = "tol")]
    Tolerance(ToleranceArgs),
}

pub fn run(cmd: ReportCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        ReportCommands::Rvm(args) => rvm::run(args, global),
        ReportCommands::Fmea(args) => fmea::run(args, global),
        ReportCommands::Bom(args) => bom::run(args, global),
        ReportCommands::TestStatus(args) => test_status::run(args, global),
        ReportCommands::OpenIssues(args) => open_issues::run(args, global),
        ReportCommands::Tolerance(args) => tolerance::run(args, global),
    }
}

// Shared helper functions

/// Parse a set of entity files, warning (once per type) about files that
/// failed to parse instead of dropping them silently.
fn parse_entity_files<T, I, P>(paths: I, type_name: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
    I: IntoIterator<Item = P>,
    P: AsRef<std::path::Path>,
{
    let mut entities = Vec::new();
    let mut skipped = 0usize;

    for path in paths {
        match tdt_core::yaml::parse_yaml_file::<T>(path.as_ref()) {
            Ok(entity) => entities.push(entity),
            Err(_) => skipped += 1,
        }
    }

    if skipped > 0 {
        eprintln!(
            "warning: skipped {} unparseable {} file(s)",
            skipped, type_name
        );
    }

    entities
}

pub(crate) fn write_output(content: &str, output_path: Option<PathBuf>) -> Result<()> {
    match output_path {
        Some(path) => {
            let file = File::create(&path).into_diagnostic()?;
            let mut writer = BufWriter::new(file);
            writer.write_all(content.as_bytes()).into_diagnostic()?;
            println!("Report written to: {}", path.display());
        }
        None => {
            print!("{}", content);
        }
    }
    Ok(())
}

/// Load all requirements using cache (no directory walk)
pub(crate) fn load_all_requirements_cached(cache: &EntityCache) -> Vec<Requirement> {
    let cached = cache.list_requirements(None, None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "requirement")
}

/// Load all requirements (legacy - uses directory walk, prefer load_all_requirements_cached)
pub(crate) fn load_all_requirements(project: &Project) -> Vec<Requirement> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_requirements_cached(&cache);
    }
    // Fallback to directory walk
    let mut requirements = Vec::new();
    let dir = project.root().join("requirements");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(req) = tdt_core::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                requirements.push(req);
            }
        }
    }

    requirements
}

/// Load all tests using cache (no directory walk)
pub(crate) fn load_all_tests_cached(cache: &EntityCache) -> Vec<Test> {
    let cached = cache.list_tests(None, None, None, None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "test")
}

/// Load all tests (legacy - uses directory walk, prefer load_all_tests_cached)
pub(crate) fn load_all_tests(project: &Project) -> Vec<Test> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_tests_cached(&cache);
    }
    // Fallback to directory walk
    let mut tests = Vec::new();

    for subdir in ["verification/protocols", "validation/protocols"] {
        let dir = project.root().join(subdir);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(test) = tdt_core::yaml::parse_yaml_file::<Test>(entry.path()) {
                    tests.push(test);
                }
            }
        }
    }

    tests
}

/// Load all results using cache (no directory walk)
pub(crate) fn load_all_results_cached(cache: &EntityCache) -> Vec<TestResult> {
    let cached = cache.list_results(None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "result")
}

/// Load all results (legacy - uses directory walk, prefer load_all_results_cached)
pub(crate) fn load_all_results(project: &Project) -> Vec<TestResult> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_results_cached(&cache);
    }
    // Fallback to directory walk
    let mut results = Vec::new();

    for subdir in ["verification/results", "validation/results"] {
        let dir = project.root().join(subdir);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(result) = tdt_core::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                    results.push(result);
                }
            }
        }
    }

    results
}

/// Load all risks using cache (no directory walk)
pub(crate) fn load_all_risks_cached(cache: &EntityCache) -> Vec<Risk> {
    let cached = cache.list_risks(None, None, None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "risk")
}

/// Load all risks (legacy - uses directory walk, prefer load_all_risks_cached)
pub(crate) fn load_all_risks(project: &Project) -> Vec<Risk> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_risks_cached(&cache);
    }
    // Fallback to directory walk
    let mut risks = Vec::new();
    let dir = project.root().join("risks");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = tdt_core::yaml::parse_yaml_file::<Risk>(entry.path()) {
                risks.push(risk);
            }
        }
    }

    risks
}

/// Load all components using cache (no directory walk)
pub(crate) fn load_all_components_cached(cache: &EntityCache) -> Vec<Component> {
    let cached = cache.list_components(None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "component")
}

/// Load all components (legacy - uses directory walk, prefer load_all_components_cached)
pub(crate) fn load_all_components(project: &Project) -> Vec<Component> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_components_cached(&cache);
    }
    // Fallback to directory walk
    let mut components = Vec::new();
    let dir = project.root().join("bom/components");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(cmp) = tdt_core::yaml::parse_yaml_file::<Component>(entry.path()) {
                components.push(cmp);
            }
        }
    }

    components
}

/// Load all assemblies using cache (no directory walk)
pub(crate) fn load_all_assemblies_cached(cache: &EntityCache) -> Vec<Assembly> {
    // Use list_entities with prefix filter since there's no specialized list_assemblies
    let cached = cache.list_entities(&tdt_core::core::cache::EntityFilter {
        prefix: Some(tdt_core::core::identity::EntityPrefix::Asm),
        ..Default::default()
    });
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "assembly")
}

/// Load all assemblies (legacy - uses directory walk, prefer load_all_assemblies_cached)
pub(crate) fn load_all_assemblies(project: &Project) -> Vec<Assembly> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_assemblies_cached(&cache);
    }
    // Fallback to directory walk
    let mut assemblies = Vec::new();
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                assemblies.push(asm);
            }
        }
    }

    assemblies
}

/// Load all quotes using cache (no directory walk)
pub(crate) fn load_all_quotes_cached(cache: &EntityCache) -> Vec<Quote> {
    let cached = cache.list_quotes(None, None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "quote")
}

/// Load all quotes (legacy - uses directory walk, prefer load_all_quotes_cached)
pub(crate) fn load_all_quotes(project: &Project) -> Vec<Quote> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_quotes_cached(&cache);
    }
    // Fallback to directory walk
    let mut quotes = Vec::new();
    let dir = project.root().join("bom/quotes");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(quote) = tdt_core::yaml::parse_yaml_file::<Quote>(entry.path()) {
                quotes.push(quote);
            }
        }
    }

    quotes
}

/// Load a specific assembly using cache (no directory walk)
pub(crate) fn load_assembly_cached(cache: &EntityCache, id: &str) -> Result<Assembly> {
    // Use cache to find the assembly
    if let Some(entity) = cache.get_entity(id) {
        if entity.prefix == "ASM" {
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(&entity.file_path) {
                return Ok(asm);
            }
        }
    }
    Err(miette::miette!("Assembly not found: {}", id))
}

/// Load a specific assembly (legacy - uses directory walk, prefer load_assembly_cached)
pub(crate) fn load_assembly(project: &Project, id: &str) -> Result<Assembly> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_assembly_cached(&cache, id);
    }
    // Fallback to directory walk
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                if asm.id.to_string() == id {
                    return Ok(asm);
                }
            }
        }
    }

    Err(miette::miette!("Assembly not found: {}", id))
}

/// Load all NCRs using cache (no directory walk)
pub(crate) fn load_all_ncrs_cached(cache: &EntityCache) -> Vec<tdt_core::entities::ncr::Ncr> {
    let cached = cache.list_ncrs(None, None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "NCR")
}

/// Load all NCRs (legacy - uses directory walk, prefer load_all_ncrs_cached)
pub(crate) fn load_all_ncrs(project: &Project) -> Vec<tdt_core::entities::ncr::Ncr> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_ncrs_cached(&cache);
    }
    // Fallback to directory walk
    let mut ncrs = Vec::new();
    let dir = project.root().join("manufacturing/ncrs");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(ncr) =
                tdt_core::yaml::parse_yaml_file::<tdt_core::entities::ncr::Ncr>(entry.path())
            {
                ncrs.push(ncr);
            }
        }
    }

    ncrs
}

/// Load all CAPAs using cache (no directory walk)
pub(crate) fn load_all_capas_cached(cache: &EntityCache) -> Vec<tdt_core::entities::capa::Capa> {
    let cached = cache.list_capas(None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "CAPA")
}

/// Load all CAPAs (legacy - uses directory walk, prefer load_all_capas_cached)
pub(crate) fn load_all_capas(project: &Project) -> Vec<tdt_core::entities::capa::Capa> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_capas_cached(&cache);
    }
    // Fallback to directory walk
    let mut capas = Vec::new();

    // CAPAs can be in quality/capas or manufacturing/capas
    for subdir in ["quality/capas", "manufacturing/capas"] {
        let dir = project.root().join(subdir);

        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(capa) =
                    tdt_core::yaml::parse_yaml_file::<tdt_core::entities::capa::Capa>(entry.path())
                {
                    capas.push(capa);
                }
            }
        }
    }

    capas
}

/// Load all features using cache (no directory walk)
pub(crate) fn load_all_features_cached(cache: &EntityCache) -> Vec<Feature> {
    let cached = cache.list_features(None, None, None, None, None, None);
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "feature")
}

/// Load all features (legacy - uses directory walk, prefer load_all_features_cached)
pub(crate) fn load_all_features(project: &Project) -> Vec<Feature> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_features_cached(&cache);
    }
    // Fallback to directory walk
    let mut features = Vec::new();
    let dir = project.root().join("tolerances/features");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(feat) = tdt_core::yaml::parse_yaml_file::<Feature>(entry.path()) {
                features.push(feat);
            }
        }
    }

    features
}

/// Load all mates using cache (no directory walk)
pub(crate) fn load_all_mates_cached(cache: &EntityCache) -> Vec<Mate> {
    // Use list_entities with prefix filter since there's no specialized list_mates
    let cached = cache.list_entities(&tdt_core::core::cache::EntityFilter {
        prefix: Some(tdt_core::core::identity::EntityPrefix::Mate),
        ..Default::default()
    });
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "mate")
}

/// Load all mates (legacy - uses directory walk, prefer load_all_mates_cached)
pub(crate) fn load_all_mates(project: &Project) -> Vec<Mate> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_mates_cached(&cache);
    }
    // Fallback to directory walk
    let mut mates = Vec::new();
    let dir = project.root().join("tolerances/mates");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(mate) = tdt_core::yaml::parse_yaml_file::<Mate>(entry.path()) {
                mates.push(mate);
            }
        }
    }

    mates
}

/// Load all stackups using cache (no directory walk)
pub(crate) fn load_all_stackups_cached(cache: &EntityCache) -> Vec<Stackup> {
    // Use list_entities with prefix filter since there's no specialized list_stackups
    let cached = cache.list_entities(&tdt_core::core::cache::EntityFilter {
        prefix: Some(tdt_core::core::identity::EntityPrefix::Tol),
        ..Default::default()
    });
    parse_entity_files(cached.into_iter().map(|c| c.file_path), "stackup")
}

/// Load all stackups (legacy - uses directory walk, prefer load_all_stackups_cached)
pub(crate) fn load_all_stackups(project: &Project) -> Vec<Stackup> {
    // Use cache if available
    if let Ok(cache) = EntityCache::open(project) {
        return load_all_stackups_cached(&cache);
    }
    // Fallback to directory walk
    let mut stackups = Vec::new();
    let dir = project.root().join("tolerances/stackups");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(stackup) = tdt_core::yaml::parse_yaml_file::<Stackup>(entry.path()) {
                stackups.push(stackup);
            }
        }
    }

    stackups
}
