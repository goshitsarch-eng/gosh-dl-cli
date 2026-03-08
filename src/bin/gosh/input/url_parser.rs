use anyhow::{bail, Result};
use std::path::PathBuf;

/// Parsed input type
#[derive(Debug)]
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
                    let end = uri[start..]
                        .find('&')
                        .map(|i| start + i)
                        .unwrap_or(uri.len());
                    let name = &uri[start..end];
                    urlencoding::decode(name)
                        .map(|s| s.into_owned())
                        .unwrap_or_else(|_| name.to_string())
                } else {
                    // Extract info hash
                    if let Some(hash_start) = uri.find("btih:") {
                        let start = hash_start + 5;
                        let end = uri[start..]
                            .find('&')
                            .map(|i| start + i)
                            .unwrap_or(uri.len());
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
        bail!(
            "Existing file is not a torrent file: {}. Use 'gosh add -i <file>' to read a URL list.",
            path.display()
        );
    }

    // If it looks like a path but doesn't exist
    if input.ends_with(".torrent") {
        bail!("Torrent file not found: {}", input);
    }

    if looks_like_implicit_url(input) {
        return Ok(ParsedInput::Http(format!("https://{}", input)));
    }

    bail!(
        "Cannot determine input type for: {}. \
         Use http(s)://... for URLs, magnet:... for magnet links, or a path to a .torrent file.",
        input
    )
}

fn looks_like_implicit_url(input: &str) -> bool {
    if looks_like_local_path(input) {
        return false;
    }

    let host_and_path = input
        .split_once('/')
        .map(|(host, _)| host)
        .unwrap_or(input);
    let host = host_and_path
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host_and_path);

    if host.is_empty() || !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return false;
    }

    if !host
        .split('.')
        .all(|label| !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return false;
    }

    let last = host.rsplit('.').next().unwrap().to_ascii_lowercase();
    !common_file_extensions().contains(&last.as_str())
}

fn looks_like_local_path(input: &str) -> bool {
    input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with('/')
        || input.starts_with("~/")
        || input.starts_with(".\\")
        || input.starts_with("..\\")
        || looks_like_windows_absolute_path(input)
}

fn looks_like_windows_absolute_path(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn common_file_extensions() -> &'static [&'static str] {
    &[
        "txt", "log", "csv", "json", "xml", "yml", "yaml", "toml", "ini", "cfg", "conf", "md",
        "rst", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "rs", "py",
        "js", "ts", "go", "c", "h", "cpp", "java", "rb", "sh", "bat",
    ]
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
    use tempfile::{Builder, NamedTempFile};

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

    #[test]
    fn test_parse_host_with_path() {
        let result = parse_input("example.com/file.zip").unwrap();
        assert!(matches!(result, ParsedInput::Http(_)));
    }

    #[test]
    fn test_parse_existing_non_torrent_file_errors() {
        let file = NamedTempFile::new().unwrap();
        let err = parse_input(file.path().to_str().unwrap()).unwrap_err();
        assert!(err
            .to_string()
            .contains("Existing file is not a torrent file"));
    }

    #[test]
    fn test_parse_existing_torrent_file_is_accepted() {
        let file = Builder::new().suffix(".torrent").tempfile().unwrap();
        let result = parse_input(file.path().to_str().unwrap()).unwrap();
        assert!(matches!(result, ParsedInput::TorrentFile(_)));
    }

    #[test]
    fn test_parse_missing_torrent_file_errors() {
        let err = parse_input("missing.torrent").unwrap_err();
        assert!(err.to_string().contains("Torrent file not found"));
    }

    #[test]
    fn test_parse_path_like_inputs_are_not_urls() {
        for input in ["./foo.bar", "../foo.bar", "/tmp/foo.bar", "~/foo.bar"] {
            assert!(parse_input(input).is_err(), "expected '{input}' to be rejected");
        }
    }
}
