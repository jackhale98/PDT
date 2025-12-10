use clap::Parser;
use miette::Result;
use pdt::cli::{Cli, Commands};

fn main() -> Result<()> {
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

    match cli.command {
        Commands::Init(args) => pdt::cli::commands::init::run(args),
        Commands::Req(cmd) => pdt::cli::commands::req::run(cmd),
        Commands::Validate(args) => pdt::cli::commands::validate::run(args),
        Commands::Link(cmd) => pdt::cli::commands::link::run(cmd),
        Commands::Trace(cmd) => pdt::cli::commands::trace::run(cmd),
    }
}
