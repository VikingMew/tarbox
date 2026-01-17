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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        assert_eq!(config.database.url, "postgres://postgres:postgres@localhost:5432/tarbox");
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.database.min_connections, 2);

        assert_eq!(config.fuse.mount_point, "/mnt/tarbox");
        assert!(!config.fuse.allow_other);

        assert!(config.audit.enabled);
        assert_eq!(config.audit.retention_days, 90);

        assert_eq!(config.cache.max_entries, 10000);
        assert_eq!(config.cache.ttl_seconds, 300);

        assert_eq!(config.api.rest_addr, "127.0.0.1:8080");
        assert_eq!(config.api.grpc_addr, "127.0.0.1:50051");
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config::default();
        let config2 = config1.clone();

        assert_eq!(config1.database.url, config2.database.url);
        assert_eq!(config1.fuse.mount_point, config2.fuse.mount_point);
        assert_eq!(config1.audit.enabled, config2.audit.enabled);
    }

    #[test]
    fn test_database_config_creation() {
        let db_config = DatabaseConfig {
            url: "postgres://user:pass@host:5432/db".to_string(),
            max_connections: 20,
            min_connections: 5,
        };

        assert_eq!(db_config.url, "postgres://user:pass@host:5432/db");
        assert_eq!(db_config.max_connections, 20);
        assert_eq!(db_config.min_connections, 5);
    }

    #[test]
    fn test_fuse_config_allow_other_flag() {
        let fuse_config = FuseConfig { mount_point: "/custom/path".to_string(), allow_other: true };

        assert_eq!(fuse_config.mount_point, "/custom/path");
        assert!(fuse_config.allow_other);
    }

    #[test]
    fn test_audit_config_disabled() {
        let audit_config = AuditConfig { enabled: false, retention_days: 30 };

        assert!(!audit_config.enabled);
        assert_eq!(audit_config.retention_days, 30);
    }

    #[test]
    fn test_cache_config_custom_values() {
        let cache_config = CacheConfig { max_entries: 50000, ttl_seconds: 600 };

        assert_eq!(cache_config.max_entries, 50000);
        assert_eq!(cache_config.ttl_seconds, 600);
    }

    #[test]
    fn test_api_config_different_addresses() {
        let api_config =
            ApiConfig { rest_addr: "0.0.0.0:80".to_string(), grpc_addr: "0.0.0.0:443".to_string() };

        assert_eq!(api_config.rest_addr, "0.0.0.0:80");
        assert_eq!(api_config.grpc_addr, "0.0.0.0:443");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "database": {
                "url": "postgres://localhost/test",
                "max_connections": 15,
                "min_connections": 3
            },
            "fuse": {
                "mount_point": "/test",
                "allow_other": true
            },
            "audit": {
                "enabled": false,
                "retention_days": 60
            },
            "cache": {
                "max_entries": 5000,
                "ttl_seconds": 120
            },
            "api": {
                "rest_addr": "localhost:8080",
                "grpc_addr": "localhost:50051"
            }
        }"#;

        let config: Result<Config, _> = serde_json::from_str(json);
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.database.max_connections, 15);
        assert!(config.fuse.allow_other);
        assert!(!config.audit.enabled);
        assert_eq!(config.cache.max_entries, 5000);
    }
}
