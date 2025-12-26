use clap::Parser;
use miette::Result;
use tdt::cli::{Cli, Commands};

fn main() -> Result<()> {
    // Reset SIGPIPE to default behavior (terminate silently) for proper Unix piping.
    // Without this, piping to `head`, `grep -q`, etc. causes a panic on broken pipe.
    // This is standard practice for CLI tools that output to stdout.
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
    // Install miette's fancy error handler for beautiful diagnostics
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .unicode(true)
                .context_lines(2)
                .tab_width(4)
                .build(),
        )
    }))?;

    let cli = Cli::parse();
    let global = cli.global;

    match cli.command {
        Commands::Init(args) => tdt::cli::commands::init::run(args),
        Commands::Req(cmd) => tdt::cli::commands::req::run(cmd, &global),
        Commands::Risk(cmd) => tdt::cli::commands::risk::run(cmd, &global),
        Commands::Test(cmd) => tdt::cli::commands::test::run(cmd, &global),
        Commands::Rslt(cmd) => tdt::cli::commands::rslt::run(cmd, &global),
        Commands::Cmp(cmd) => tdt::cli::commands::cmp::run(cmd, &global),
        Commands::Asm(cmd) => tdt::cli::commands::asm::run(cmd, &global),
        Commands::Quote(cmd) => tdt::cli::commands::quote::run(cmd, &global),
        Commands::Sup(cmd) => tdt::cli::commands::sup::run(cmd, &global),
        Commands::Proc(cmd) => tdt::cli::commands::proc::run(cmd, &global),
        Commands::Ctrl(cmd) => tdt::cli::commands::ctrl::run(cmd, &global),
        Commands::Work(cmd) => tdt::cli::commands::work::run(cmd, &global),
        Commands::Lot(cmd) => tdt::cli::commands::lot::run(cmd, &global),
        Commands::Dev(cmd) => tdt::cli::commands::dev::run(cmd, &global),
        Commands::Ncr(cmd) => tdt::cli::commands::ncr::run(cmd, &global),
        Commands::Capa(cmd) => tdt::cli::commands::capa::run(cmd, &global),
        Commands::Feat(cmd) => tdt::cli::commands::feat::run(cmd, &global),
        Commands::Mate(cmd) => tdt::cli::commands::mate::run(cmd, &global),
        Commands::Tol(cmd) => tdt::cli::commands::tol::run(cmd, &global),
        Commands::Validate(args) => tdt::cli::commands::validate::run(args),
        Commands::Link(cmd) => tdt::cli::commands::link::run(cmd),
        Commands::Trace(cmd) => tdt::cli::commands::trace::run(cmd, &global),
        Commands::Dsm(args) => tdt::cli::commands::dsm::run(args, &global),
        Commands::Dmm(args) => tdt::cli::commands::dmm::run(args, &global),
        Commands::Report(cmd) => tdt::cli::commands::report::run(cmd, &global),
        Commands::WhereUsed(args) => tdt::cli::commands::where_used::run(args, &global),
        Commands::History(args) => tdt::cli::commands::history::run(args),
        Commands::Blame(args) => tdt::cli::commands::blame::run(args),
        Commands::Diff(args) => tdt::cli::commands::diff::run(args),
        Commands::Baseline(cmd) => tdt::cli::commands::baseline::run(cmd),
        Commands::Submit(args) => args.run(&global),
        Commands::Approve(args) => args.run(&global),
        Commands::Reject(args) => args.run(&global),
        Commands::Release(args) => args.run(&global),
        Commands::Review(cmd) => cmd.run(&global),
        Commands::Team(cmd) => cmd.run(&global),
        Commands::Import(args) => tdt::cli::commands::import::run(args),
        Commands::Bulk(cmd) => tdt::cli::commands::bulk::run(cmd),
        Commands::Status(args) => tdt::cli::commands::status::run(args, &global),
        Commands::Cache(cmd) => tdt::cli::commands::cache::run(cmd),
        Commands::Config(cmd) => tdt::cli::commands::config::run(cmd, &global),
        Commands::Search(args) => tdt::cli::commands::search::run(args, &global),
        Commands::Recent(args) => tdt::cli::commands::recent::run(args, &global),
        Commands::Tags(cmd) => tdt::cli::commands::tags::run(cmd, &global),
        Commands::Schema(cmd) => tdt::cli::commands::schema::run(cmd),
        Commands::Completions(args) => tdt::cli::commands::completions::run(args),
    }
}
