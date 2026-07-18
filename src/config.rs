use std::env;

use crate::error::{AstralError, Result};

const DEFAULT_LOG_FILTER: &str = "astral=info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub log_filter: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_filter: DEFAULT_LOG_FILTER.to_owned(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let log_filter = match env::var("RUST_LOG") {
            Ok(value) if value.trim().is_empty() => {
                return Err(AstralError::InvalidConfiguration {
                    message: "RUST_LOG must not be empty".to_owned(),
                });
            }
            Ok(value) => value,
            Err(env::VarError::NotPresent) => DEFAULT_LOG_FILTER.to_owned(),
            Err(env::VarError::NotUnicode(_)) => {
                return Err(AstralError::InvalidConfiguration {
                    message: "RUST_LOG must be valid UTF-8".to_owned(),
                });
            }
        };

        Ok(Self { log_filter })
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn default_config_has_a_safe_log_filter() {
        assert_eq!(Config::default().log_filter, "astral=info");
    }
}
