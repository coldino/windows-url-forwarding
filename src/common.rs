use std::io;
use std::path::PathBuf;

pub const PIPE_NAME: &str = r"\\.\pipe\url_ferry";
pub const NUL_TERMINATOR: u8 = 0;

#[derive(Debug)]
pub enum UrlFerryError {
    Io(io::Error),
    InvalidUrl(String),
    PipeError(String),
    RegistryError(String),
    LaunchError(String),
}

impl From<io::Error> for UrlFerryError {
    fn from(err: io::Error) -> Self {
        UrlFerryError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, UrlFerryError>;

/// Validates that a URL is http or https
pub fn validate_url(url: &str) -> Result<()> {
    let url_lower = url.to_lowercase();
    if url_lower.starts_with("http://") || url_lower.starts_with("https://") {
        Ok(())
    } else {
        Err(UrlFerryError::InvalidUrl(format!(
            "URL must start with http:// or https://: {}",
            url
        )))
    }
}

/// Gets the path to the current executable
pub fn get_exe_path() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| UrlFerryError::Io(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_http() {
        assert!(validate_url("http://example.com").is_ok());
    }

    #[test]
    fn test_validate_url_https() {
        assert!(validate_url("https://example.com").is_ok());
    }

    #[test]
    fn test_validate_url_uppercase() {
        assert!(validate_url("HTTP://EXAMPLE.COM").is_ok());
    }

    #[test]
    fn test_validate_url_invalid() {
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("file:///c:/path").is_err());
        assert!(validate_url("not a url").is_err());
    }
}
