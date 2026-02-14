use anyhow::Result;
use clap::{CommandFactory, Parser};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod app;
mod cli;
mod commands;
mod config;
mod direct;
mod format;
mod input;
mod output;
#[cfg(feature = "tui")]
mod tui;
mod util;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(code) => code,
        Err(e) => {
            format::print_error(&format!("{e:#}"));
            1
        }
    };
    std::process::exit(code);
}

async fn run() -> Result<i32> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize color output
    format::init_color(match cli.color {
        cli::ColorChoice::Auto => None,
        cli::ColorChoice::Always => Some(true),
        cli::ColorChoice::Never => Some(false),
    });

    // Handle completions early (no engine needed)
    if let Some(Commands::Completions(ref args)) = cli.command {
        clap_complete::generate(
            args.shell,
            &mut Cli::command(),
            "gosh",
            &mut std::io::stdout(),
        );
        return Ok(0);
    }

    // Setup logging based on verbosity
    setup_logging(cli.verbose, cli.quiet)?;

    // Load config file
    let mut config = config::CliConfig::load(cli.config.as_deref())?;

    // Validate configuration values
    config.validate()?;

    // Apply environment variable overrides first
    config.apply_env_overrides();

    // Apply CLI overrides to config (CLI takes precedence)
    if cli.no_dht {
        config.engine.enable_dht = false;
    }
    if cli.no_pex {
        config.engine.enable_pex = false;
    }
    if cli.no_lpd {
        config.engine.enable_lpd = false;
    }
    if let Some(n) = cli.max_peers {
        config.engine.max_peers = n;
    }
    if let Some(r) = cli.max_retries {
        config.engine.max_retries = r;
    }
    if let Some(ref proxy) = cli.proxy {
        config.engine.proxy_url = Some(proxy.clone());
    }
    if cli.insecure {
        config.engine.accept_invalid_certs = true;
        format::print_warning("TLS certificate verification disabled");
    }

    // Route to appropriate handler
    if let Some(cmd) = cli.command {
        // Subcommand provided - run it
        run_command(cmd, config, cli.output, cli.config.clone()).await?;
        Ok(0)
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
        #[cfg(feature = "tui")]
        {
            run_tui(config).await?;
            Ok(0)
        }
        #[cfg(not(feature = "tui"))]
        {
            format::print_error("TUI not available. Pass URLs to download directly.");
            Ok(1)
        }
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
    config_path: Option<std::path::PathBuf>,
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
        Commands::Config(args) => {
            commands::config::execute(args, &app.config, config_path.as_deref()).await
        }
        Commands::Completions(_) => Ok(()), // handled before engine init
    }
}

#[cfg(feature = "tui")]
async fn run_tui(config: config::CliConfig) -> Result<()> {
    let mut tui_app = tui::TuiApp::new(config).await?;
    tui_app.run().await
}
