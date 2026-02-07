use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self { version: 1 }
    }
}
