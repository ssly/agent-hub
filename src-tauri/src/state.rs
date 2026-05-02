use std::sync::Mutex;

use crate::config::Config;
use crate::i18n::Locale;
use crate::platform::Platform;

pub struct AppState {
    pub config: Config,
    pub locale: Locale,
    pub platforms: Vec<Platform>,
}

impl AppState {
    pub fn new() -> Self {
        let config = Config::load();
        let locale = Locale::from_config(config.resolved_language());
        let platforms = crate::platform::discover_platforms(&config);
        Self {
            config,
            locale,
            platforms,
        }
    }
}

pub type SafeState = Mutex<AppState>;
