use anyhow::Result;
use serde::Serialize;

use crate::app::App;
use crate::cli::OutputFormat;
use crate::format::format_speed;

#[derive(Serialize)]
struct GlobalStats {
    num_active: usize,
    num_waiting: usize,
    num_stopped: usize,
    download_speed: u64,
    upload_speed: u64,
    download_speed_formatted: String,
    upload_speed_formatted: String,
}

pub async fn execute(app: &App, output: OutputFormat) -> Result<()> {
    let stats = app.engine().global_stats();

    let formatted = GlobalStats {
        num_active: stats.num_active,
        num_waiting: stats.num_waiting,
        num_stopped: stats.num_stopped,
        download_speed: stats.download_speed,
        upload_speed: stats.upload_speed,
        download_speed_formatted: format!("{}/s", format_speed(stats.download_speed)),
        upload_speed_formatted: format!("{}/s", format_speed(stats.upload_speed)),
    };

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&formatted)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(&formatted)?);
        }
        OutputFormat::Table => {
            println!("Global Statistics");
            println!("=================");
            println!();
            println!("Downloads:");
            println!("  Active:   {}", stats.num_active);
            println!("  Waiting:  {}", stats.num_waiting);
            println!("  Stopped:  {}", stats.num_stopped);
            println!(
                "  Total:    {}",
                stats.num_active + stats.num_waiting + stats.num_stopped
            );
            println!();
            println!("Speed:");
            println!("  Download: {}/s", format_speed(stats.download_speed));
            println!("  Upload:   {}/s", format_speed(stats.upload_speed));
        }
    }

    Ok(())
}
