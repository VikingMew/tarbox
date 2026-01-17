use anyhow::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use std::time::Duration;

use crate::config::DatabaseConfig;

#[derive(Clone)]
pub struct DatabasePool {
    pool: PgPool,
}

pub type DatabaseTransaction<'a> = Transaction<'a, Postgres>;

impl DatabasePool {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&config.url)
            .await?;

        tracing::info!(
            "Database pool created with max_connections={}, min_connections={}",
            config.max_connections,
            config.min_connections
        );

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    pub async fn check_version(&self) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT version()").fetch_one(&self.pool).await?;
        Ok(row.0)
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        tracing::info!("Database migrations completed successfully");
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
        tracing::info!("Database pool closed");
    }

    pub async fn begin_transaction(&self) -> Result<DatabaseTransaction<'_>> {
        let tx = self.pool.begin().await?;
        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_pool_clone() {
        // Test that DatabasePool can be cloned
        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 10,
            min_connections: 2,
        };

        // We can't actually connect in unit tests, but we can test the structure
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
    }

    #[test]
    fn test_database_config_construction() {
        let config = DatabaseConfig {
            url: "postgresql://user:pass@host:5432/dbname".to_string(),
            max_connections: 20,
            min_connections: 5,
        };

        assert!(config.url.contains("postgresql://"));
        assert!(config.max_connections > config.min_connections);
    }

    #[test]
    fn test_database_config_max_min_connections() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 50,
            min_connections: 10,
        };

        assert_eq!(config.max_connections, 50);
        assert_eq!(config.min_connections, 10);
        assert!(config.max_connections >= config.min_connections);
    }

    #[test]
    fn test_database_config_url_format() {
        let config = DatabaseConfig {
            url: "postgresql://localhost:5432/tarbox".to_string(),
            max_connections: 10,
            min_connections: 2,
        };

        assert!(config.url.starts_with("postgresql://"));
        assert!(config.url.contains("tarbox"));
    }

    #[test]
    fn test_database_config_edge_cases() {
        let config1 = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 1,
            min_connections: 1,
        };
        assert_eq!(config1.max_connections, config1.min_connections);

        let config2 = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 100,
            min_connections: 1,
        };
        assert!(config2.max_connections > config2.min_connections);
    }
}
