use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod app;
mod cli;
mod commands;
mod config;
mod direct;
mod input;
mod output;
mod tui;
mod util;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize color-eyre for pretty error reports
    color_eyre::install().ok();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Setup logging based on verbosity
    setup_logging(cli.verbose, cli.quiet)?;

    // Load config file
    let config = config::CliConfig::load(cli.config.as_deref())?;

    // Route to appropriate handler
    if let Some(cmd) = cli.command {
        // Subcommand provided - run it
        run_command(cmd, config, cli.output).await
    } else if !cli.urls.is_empty() {
        // URLs provided without subcommand - direct download mode
        let opts = direct::DirectOptions {
            urls: cli.urls,
            dir: cli.dir,
            out: cli.out,
            headers: cli.headers,
            user_agent: cli.user_agent,
            referer: cli.referer,
            cookies: cli.cookies,
            checksum: cli.checksum,
            max_connections: cli.max_connections,
            max_speed: cli.max_speed,
            sequential: cli.sequential,
            select_files: cli.select_files,
            seed_ratio: cli.seed_ratio,
        };
        direct::execute(opts, config).await
    } else {
        // No URLs and no subcommand - launch TUI
        run_tui(config).await
    }
}

fn setup_logging(verbose: u8, quiet: bool) -> Result<()> {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    Ok(())
}

async fn run_command(
    cmd: Commands,
    config: config::CliConfig,
    output_format: cli::OutputFormat,
) -> Result<()> {
    // Initialize the application (engine)
    let app = app::App::new(config).await?;

    match cmd {
        Commands::Add(args) => commands::add::execute(*args, &app, output_format).await,
        Commands::List(args) => commands::list::execute(args, &app, output_format).await,
        Commands::Status(args) => commands::status::execute(args, &app, output_format).await,
        Commands::Pause(args) => commands::pause::execute(args, &app).await,
        Commands::Resume(args) => commands::resume::execute(args, &app).await,
        Commands::Cancel(args) => commands::cancel::execute(args, &app).await,
        Commands::Priority(args) => commands::priority::execute(args, &app).await,
        Commands::Stats => commands::stats::execute(&app, output_format).await,
        Commands::Info(args) => commands::info::execute(args, output_format).await,
        Commands::Config(args) => commands::config::execute(args, &app.config).await,
    }
}

async fn run_tui(config: config::CliConfig) -> Result<()> {
    let mut tui_app = tui::TuiApp::new(config).await?;
    tui_app.run().await
}
