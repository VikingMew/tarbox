use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub fuse: FuseConfig,
    pub audit: AuditConfig,
    pub cache: CacheConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuseConfig {
    pub mount_point: String,
    pub allow_other: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub rest_addr: String,
    pub grpc_addr: String,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("TARBOX"))
            .build()?;

        Ok(config.try_deserialize()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/tarbox".to_string(),
                max_connections: 10,
                min_connections: 2,
            },
            fuse: FuseConfig { mount_point: "/mnt/tarbox".to_string(), allow_other: false },
            audit: AuditConfig { enabled: true, retention_days: 90 },
            cache: CacheConfig { max_entries: 10000, ttl_seconds: 300 },
            api: ApiConfig {
                rest_addr: "127.0.0.1:8080".to_string(),
                grpc_addr: "127.0.0.1:50051".to_string(),
            },
        }
    }
}
