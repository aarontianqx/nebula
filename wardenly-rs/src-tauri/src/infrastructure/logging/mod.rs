use tracing_subscriber::{fmt, EnvFilter};

pub fn setup(is_production: bool) {
    let filter = if is_production {
        EnvFilter::new("info")
    } else {
        EnvFilter::new("debug")
    };

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    tracing::info!("Logging initialized");
}

