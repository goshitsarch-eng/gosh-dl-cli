use anyhow::Result;
use serde::Serialize;

pub fn print_json<T: Serialize>(value: &T, pretty: bool) -> Result<()> {
    let output = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    println!("{}", output);
    Ok(())
}
