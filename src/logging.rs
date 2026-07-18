use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{
    config::Config,
    error::{AstralError, Result},
};

pub fn init(config: &Config) -> Result<()> {
    let filter = EnvFilter::try_new(&config.log_filter).map_err(|error| AstralError::Logging {
        message: error.to_string(),
    })?;

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .try_init()
        .map_err(|error| AstralError::Logging {
            message: error.to_string(),
        })
}
