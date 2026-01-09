use anyhow::{bail, Result};
use std::path::PathBuf;

/// Parsed input type
pub enum ParsedInput {
    /// HTTP or HTTPS URL
    Http(String),
    /// Magnet link
    Magnet(String),
    /// Path to a .torrent file
    TorrentFile(PathBuf),
}

impl ParsedInput {
    /// Get a display string for this input
    pub fn display(&self) -> String {
        match self {
            ParsedInput::Http(url) => url.clone(),
            ParsedInput::Magnet(uri) => {
                // Extract display name from magnet if available
                if let Some(dn_start) = uri.find("dn=") {
                    let start = dn_start + 3;
                    let end = uri[start..].find('&').map(|i| start + i).unwrap_or(uri.len());
                    let name = &uri[start..end];
                    urlencoding::decode(name)
                        .map(|s| s.into_owned())
                        .unwrap_or_else(|_| name.to_string())
                } else {
                    // Extract info hash
                    if let Some(hash_start) = uri.find("btih:") {
                        let start = hash_start + 5;
                        let end = uri[start..].find('&').map(|i| start + i).unwrap_or(uri.len());
                        format!("magnet:{}", &uri[start..end.min(start + 16)])
                    } else {
                        uri.clone()
                    }
                }
            }
            ParsedInput::TorrentFile(path) => path.display().to_string(),
        }
    }

    /// Get the kind of this input as a string
    pub fn kind(&self) -> &'static str {
        match self {
            ParsedInput::Http(_) => "http",
            ParsedInput::Magnet(_) => "magnet",
            ParsedInput::TorrentFile(_) => "torrent",
        }
    }
}

/// Parse an input string into a typed input
pub fn parse_input(input: &str) -> Result<ParsedInput> {
    let input = input.trim();

    if input.is_empty() {
        bail!("Empty input");
    }

    // Check for magnet links
    if input.starts_with("magnet:") {
        return Ok(ParsedInput::Magnet(input.to_string()));
    }

    // Check for HTTP/HTTPS URLs
    if input.starts_with("http://") || input.starts_with("https://") {
        return Ok(ParsedInput::Http(input.to_string()));
    }

    // Check for file paths
    let path = PathBuf::from(input);
    if path.exists() {
        if input.ends_with(".torrent") || is_torrent_file(&path) {
            return Ok(ParsedInput::TorrentFile(path));
        }
        // If it exists but isn't a torrent, try to use it as a URL list file?
        // For now, assume it's a torrent file
        return Ok(ParsedInput::TorrentFile(path));
    }

    // If it looks like a path but doesn't exist
    if input.ends_with(".torrent") {
        bail!("Torrent file not found: {}", input);
    }

    // If it looks like a URL without protocol, assume HTTPS
    if input.contains('.') && !input.contains('/') || input.starts_with("www.") {
        return Ok(ParsedInput::Http(format!("https://{}", input)));
    }

    // Try adding https:// prefix if it contains slashes (likely a URL)
    if input.contains('/') && input.contains('.') {
        return Ok(ParsedInput::Http(format!("https://{}", input)));
    }

    bail!(
        "Cannot determine input type for: {}. \
         Use http(s)://... for URLs, magnet:... for magnet links, or a path to a .torrent file.",
        input
    )
}

/// Check if a file is likely a torrent file by reading magic bytes
fn is_torrent_file(path: &PathBuf) -> bool {
    use std::fs::File;
    use std::io::Read;

    if let Ok(mut file) = File::open(path) {
        let mut buf = [0u8; 11];
        if file.read_exact(&mut buf).is_ok() {
            // Torrent files start with "d8:announce" or similar bencode
            return buf[0] == b'd' && buf[1].is_ascii_digit();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_url() {
        let result = parse_input("https://example.com/file.zip").unwrap();
        assert!(matches!(result, ParsedInput::Http(_)));
    }

    #[test]
    fn test_parse_magnet() {
        let result = parse_input("magnet:?xt=urn:btih:abc123").unwrap();
        assert!(matches!(result, ParsedInput::Magnet(_)));
    }

    #[test]
    fn test_parse_bare_domain() {
        let result = parse_input("example.com").unwrap();
        if let ParsedInput::Http(url) = result {
            assert!(url.starts_with("https://"));
        } else {
            panic!("Expected HTTP");
        }
    }
}
