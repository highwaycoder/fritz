use std::io::Read;
use crate::FallbackChain;

pub struct Config {
    pub max_connections: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_connections: 128,
        }
    }
}

impl Config {
    pub fn read(paths: FallbackChain) -> Self {
        let config = Self::default();
        for mut file in paths {
            let mut config_file_contents = String::new();
            file.read_to_string(&mut config_file_contents).expect("Could not read config file");
        }
        config
    }
}
