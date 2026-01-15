use anyhow::{Context, Result};
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
            .await
            .context("Failed to create database pool")?;

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
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .context("Database health check failed")?;
        Ok(())
    }

    pub async fn check_version(&self) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT version()")
            .fetch_one(&self.pool)
            .await
            .context("Failed to get PostgreSQL version")?;
        Ok(row.0)
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        tracing::info!("Database migrations completed successfully");
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
        tracing::info!("Database pool closed");
    }

    pub async fn begin_transaction(&self) -> Result<DatabaseTransaction<'_>> {
        let tx = self.pool.begin().await.context("Failed to begin transaction")?;
        Ok(tx)
    }
}
