// WASI configuration management

use serde::{Deserialize, Serialize};
use std::env;

/// Database connection mode for WASI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DbMode {
    /// HTTP mode: connect to Tarbox API server via HTTP
    #[default]
    Http,
    /// SQLite mode: use embedded SQLite database (if feature enabled)
    #[cfg(feature = "sqlite")]
    Sqlite,
}

impl std::fmt::Display for DbMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbMode::Http => write!(f, "http"),
            #[cfg(feature = "sqlite")]
            DbMode::Sqlite => write!(f, "sqlite"),
        }
    }
}

impl std::str::FromStr for DbMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http" => Ok(DbMode::Http),
            #[cfg(feature = "sqlite")]
            "sqlite" => Ok(DbMode::Sqlite),
            _ => Err(format!("Invalid DB mode: {}", s)),
        }
    }
}

/// WASI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasiConfig {
    /// Database connection mode
    pub db_mode: DbMode,

    /// API server URL (for HTTP mode)
    pub api_url: Option<String>,

    /// API authentication key (for HTTP mode)
    pub api_key: Option<String>,

    /// SQLite database path (for SQLite mode)
    #[cfg(feature = "sqlite")]
    pub sqlite_path: Option<String>,

    /// Cache size in MB
    pub cache_size_mb: usize,

    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,

    /// Tenant ID to use
    pub tenant_id: Option<uuid::Uuid>,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            db_mode: DbMode::Http,
            api_url: None,
            api_key: None,
            #[cfg(feature = "sqlite")]
            sqlite_path: None,
            cache_size_mb: 100,
            cache_ttl_secs: 300,
            tenant_id: None,
        }
    }
}

impl WasiConfig {
    /// Create a new WasiConfig from environment variables
    ///
    /// Environment variables:
    /// - `TARBOX_DB_MODE`: Database mode (http or sqlite)
    /// - `TARBOX_API_URL`: API server URL
    /// - `TARBOX_API_KEY`: API authentication key
    /// - `TARBOX_SQLITE_PATH`: SQLite database path
    /// - `TARBOX_CACHE_SIZE`: Cache size in MB
    /// - `TARBOX_CACHE_TTL`: Cache TTL in seconds
    /// - `TARBOX_TENANT_ID`: Tenant ID
    pub fn from_env() -> Result<Self, String> {
        let db_mode =
            env::var("TARBOX_DB_MODE").ok().and_then(|s| s.parse().ok()).unwrap_or(DbMode::Http);

        let api_url = env::var("TARBOX_API_URL").ok();
        let api_key = env::var("TARBOX_API_KEY").ok();

        #[cfg(feature = "sqlite")]
        let sqlite_path = env::var("TARBOX_SQLITE_PATH").ok();

        let cache_size_mb =
            env::var("TARBOX_CACHE_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(100);

        let cache_ttl_secs =
            env::var("TARBOX_CACHE_TTL").ok().and_then(|s| s.parse().ok()).unwrap_or(300);

        let tenant_id =
            env::var("TARBOX_TENANT_ID").ok().and_then(|s| uuid::Uuid::parse_str(&s).ok());

        // Validate configuration
        if db_mode == DbMode::Http && api_url.is_none() {
            return Err("TARBOX_API_URL is required for HTTP mode".to_string());
        }

        #[cfg(feature = "sqlite")]
        if db_mode == DbMode::Sqlite && sqlite_path.is_none() {
            return Err("TARBOX_SQLITE_PATH is required for SQLite mode".to_string());
        }

        Ok(Self {
            db_mode,
            api_url,
            api_key,
            #[cfg(feature = "sqlite")]
            sqlite_path,
            cache_size_mb,
            cache_ttl_secs,
            tenant_id,
        })
    }

    /// Create a config for HTTP mode
    pub fn http(api_url: String, api_key: Option<String>) -> Self {
        Self {
            db_mode: DbMode::Http,
            api_url: Some(api_url),
            api_key,
            #[cfg(feature = "sqlite")]
            sqlite_path: None,
            cache_size_mb: 100,
            cache_ttl_secs: 300,
            tenant_id: None,
        }
    }

    /// Create a config for SQLite mode
    #[cfg(feature = "sqlite")]
    pub fn sqlite(sqlite_path: String) -> Self {
        Self {
            db_mode: DbMode::Sqlite,
            api_url: None,
            api_key: None,
            sqlite_path: Some(sqlite_path),
            cache_size_mb: 100,
            cache_ttl_secs: 300,
            tenant_id: None,
        }
    }

    /// Set the tenant ID
    pub fn with_tenant_id(mut self, tenant_id: uuid::Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Set the cache size
    pub fn with_cache_size(mut self, size_mb: usize) -> Self {
        self.cache_size_mb = size_mb;
        self
    }

    /// Set the cache TTL
    pub fn with_cache_ttl(mut self, ttl_secs: u64) -> Self {
        self.cache_ttl_secs = ttl_secs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_db_mode_default() {
        assert_eq!(DbMode::default(), DbMode::Http);
    }

    #[test]
    fn test_db_mode_display() {
        assert_eq!(DbMode::Http.to_string(), "http");
        #[cfg(feature = "sqlite")]
        assert_eq!(DbMode::Sqlite.to_string(), "sqlite");
    }

    #[test]
    fn test_db_mode_from_str() {
        assert_eq!("http".parse::<DbMode>().unwrap(), DbMode::Http);
        assert_eq!("HTTP".parse::<DbMode>().unwrap(), DbMode::Http);
        #[cfg(feature = "sqlite")]
        {
            assert_eq!("sqlite".parse::<DbMode>().unwrap(), DbMode::Sqlite);
            assert_eq!("SQLITE".parse::<DbMode>().unwrap(), DbMode::Sqlite);
        }
        assert!("invalid".parse::<DbMode>().is_err());
    }

    #[test]
    fn test_wasi_config_default() {
        let config = WasiConfig::default();
        assert_eq!(config.db_mode, DbMode::Http);
        assert_eq!(config.cache_size_mb, 100);
        assert_eq!(config.cache_ttl_secs, 300);
        assert!(config.api_url.is_none());
        assert!(config.api_key.is_none());
        assert!(config.tenant_id.is_none());
    }

    #[test]
    fn test_wasi_config_http() {
        let config =
            WasiConfig::http("https://api.tarbox.io".to_string(), Some("api-key".to_string()));
        assert_eq!(config.db_mode, DbMode::Http);
        assert_eq!(config.api_url, Some("https://api.tarbox.io".to_string()));
        assert_eq!(config.api_key, Some("api-key".to_string()));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_wasi_config_sqlite() {
        let config = WasiConfig::sqlite("/tmp/tarbox.db".to_string());
        assert_eq!(config.db_mode, DbMode::Sqlite);
        assert_eq!(config.sqlite_path, Some("/tmp/tarbox.db".to_string()));
    }

    #[test]
    fn test_wasi_config_with_tenant_id() {
        let tenant_id = uuid::Uuid::new_v4();
        let config = WasiConfig::default().with_tenant_id(tenant_id);
        assert_eq!(config.tenant_id, Some(tenant_id));
    }

    #[test]
    fn test_wasi_config_with_cache_size() {
        let config = WasiConfig::default().with_cache_size(200);
        assert_eq!(config.cache_size_mb, 200);
    }

    #[test]
    fn test_wasi_config_with_cache_ttl() {
        let config = WasiConfig::default().with_cache_ttl(600);
        assert_eq!(config.cache_ttl_secs, 600);
    }

    #[test]
    fn test_wasi_config_builder_pattern() {
        let tenant_id = uuid::Uuid::new_v4();
        let config = WasiConfig::http("https://api.tarbox.io".to_string(), None)
            .with_tenant_id(tenant_id)
            .with_cache_size(200)
            .with_cache_ttl(600);

        assert_eq!(config.db_mode, DbMode::Http);
        assert_eq!(config.api_url, Some("https://api.tarbox.io".to_string()));
        assert_eq!(config.tenant_id, Some(tenant_id));
        assert_eq!(config.cache_size_mb, 200);
        assert_eq!(config.cache_ttl_secs, 600);
    }

    #[test]
    #[serial]
    fn test_wasi_config_from_env_http() {
        // Set environment variables
        unsafe {
            env::set_var("TARBOX_DB_MODE", "http");
            env::set_var("TARBOX_API_URL", "https://api.tarbox.io");
            env::set_var("TARBOX_API_KEY", "test-key");
            env::set_var("TARBOX_CACHE_SIZE", "200");
            env::set_var("TARBOX_CACHE_TTL", "600");
        }

        let config = WasiConfig::from_env().unwrap();
        assert_eq!(config.db_mode, DbMode::Http);
        assert_eq!(config.api_url, Some("https://api.tarbox.io".to_string()));
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.cache_size_mb, 200);
        assert_eq!(config.cache_ttl_secs, 600);

        // Cleanup
        unsafe {
            env::remove_var("TARBOX_DB_MODE");
            env::remove_var("TARBOX_API_URL");
            env::remove_var("TARBOX_API_KEY");
            env::remove_var("TARBOX_CACHE_SIZE");
            env::remove_var("TARBOX_CACHE_TTL");
        }
    }

    #[test]
    #[serial]
    fn test_wasi_config_from_env_missing_api_url() {
        unsafe {
            env::set_var("TARBOX_DB_MODE", "http");
            env::remove_var("TARBOX_API_URL");
        }

        let result = WasiConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("TARBOX_API_URL"));

        unsafe {
            env::remove_var("TARBOX_DB_MODE");
        }
    }

    #[test]
    #[serial]
    fn test_wasi_config_from_env_defaults() {
        // Clear all env vars
        unsafe {
            env::remove_var("TARBOX_DB_MODE");
            env::remove_var("TARBOX_API_URL");
            env::remove_var("TARBOX_API_KEY");
            env::remove_var("TARBOX_CACHE_SIZE");
            env::remove_var("TARBOX_CACHE_TTL");

            // Set minimal required
            env::set_var("TARBOX_API_URL", "https://api.tarbox.io");
        }

        let config = WasiConfig::from_env().unwrap();
        assert_eq!(config.db_mode, DbMode::Http); // Default
        assert_eq!(config.cache_size_mb, 100); // Default
        assert_eq!(config.cache_ttl_secs, 300); // Default

        unsafe {
            env::remove_var("TARBOX_API_URL");
        }
    }

    #[test]
    #[serial]
    fn test_wasi_config_from_env_with_tenant_id() {
        let tenant_id = uuid::Uuid::new_v4();
        unsafe {
            env::set_var("TARBOX_API_URL", "https://api.tarbox.io");
            env::set_var("TARBOX_TENANT_ID", tenant_id.to_string());
        }

        let config = WasiConfig::from_env().unwrap();
        assert_eq!(config.tenant_id, Some(tenant_id));

        unsafe {
            env::remove_var("TARBOX_API_URL");
            env::remove_var("TARBOX_TENANT_ID");
        }
    }

    #[test]
    fn test_wasi_config_clone() {
        let config = WasiConfig::http("https://api.tarbox.io".to_string(), None);
        let cloned = config.clone();
        assert_eq!(config.api_url, cloned.api_url);
        assert_eq!(config.db_mode, cloned.db_mode);
    }

    #[test]
    fn test_db_mode_equality() {
        assert_eq!(DbMode::Http, DbMode::Http);
        #[cfg(feature = "sqlite")]
        {
            assert_eq!(DbMode::Sqlite, DbMode::Sqlite);
            assert_ne!(DbMode::Http, DbMode::Sqlite);
        }
    }

    #[test]
    fn test_db_mode_clone() {
        let mode = DbMode::Http;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_wasi_config_serialization() {
        let config = WasiConfig::http("https://api.tarbox.io".to_string(), Some("key".to_string()));
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("http"));
        assert!(json.contains("https://api.tarbox.io"));
    }

    #[test]
    fn test_wasi_config_deserialization() {
        let json = r#"{"db_mode":"Http","api_url":"https://api.tarbox.io","api_key":"key","cache_size_mb":100,"cache_ttl_secs":300,"tenant_id":null}"#;
        let config: WasiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.db_mode, DbMode::Http);
        assert_eq!(config.api_url, Some("https://api.tarbox.io".to_string()));
    }
}
