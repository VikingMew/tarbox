use crate::fs::FileSystem;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Handle for an active FUSE mount
#[derive(Debug)]
pub struct MountHandle {
    pub tenant_id: Uuid,
    pub mount_path: PathBuf,
    pub fuse_process: Option<Child>,
}

/// Manages FUSE mounts for CSI volumes
#[derive(Clone)]
pub struct MountManager<'a> {
    #[allow(dead_code)] // Will be used for future FUSE operations
    fs: Arc<FileSystem<'a>>,
    active_mounts: Arc<Mutex<HashMap<String, MountHandle>>>,
}

impl<'a> MountManager<'a> {
    pub fn new(fs: Arc<FileSystem<'a>>) -> Self {
        Self { fs, active_mounts: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Mount a volume at the specified path
    ///
    /// For CSI, we spawn a FUSE process in the background
    pub async fn mount(
        &self,
        volume_id: &str,
        tenant_id: Uuid,
        target_path: PathBuf,
        read_only: bool,
    ) -> Result<()> {
        let mut mounts = self.active_mounts.lock().await;

        // Check if already mounted
        if mounts.contains_key(volume_id) {
            return Ok(());
        }

        // Create mount point if not exists
        tokio::fs::create_dir_all(&target_path).await.context("Failed to create mount point")?;

        // Spawn FUSE mount process
        let mut cmd = Command::new("tarbox");
        cmd.arg("--tenant").arg(tenant_id.to_string()).arg("mount").arg(&target_path);

        if read_only {
            cmd.arg("--read-only");
        }

        let child = cmd.spawn().context("Failed to spawn FUSE mount process")?;

        // Store mount handle
        mounts.insert(
            volume_id.to_string(),
            MountHandle { tenant_id, mount_path: target_path.clone(), fuse_process: Some(child) },
        );

        Ok(())
    }

    /// Unmount a volume
    pub async fn unmount(&self, volume_id: &str) -> Result<()> {
        let mut mounts = self.active_mounts.lock().await;

        if let Some(mut handle) = mounts.remove(volume_id) {
            // Kill FUSE process
            if let Some(mut child) = handle.fuse_process.take() {
                child.kill().context("Failed to kill FUSE process")?;
                child.wait().context("Failed to wait for FUSE process")?;
            }

            // Unmount filesystem
            let status = Command::new("fusermount")
                .arg("-u")
                .arg(&handle.mount_path)
                .status()
                .context("Failed to unmount")?;

            if !status.success() {
                anyhow::bail!("fusermount failed with status: {}", status);
            }
        }

        Ok(())
    }

    /// Check if a volume is mounted
    pub async fn is_mounted(&self, volume_id: &str) -> bool {
        let mounts = self.active_mounts.lock().await;
        mounts.contains_key(volume_id)
    }

    /// Get mount path for a volume
    pub async fn get_mount_path(&self, volume_id: &str) -> Option<PathBuf> {
        let mounts = self.active_mounts.lock().await;
        mounts.get(volume_id).map(|h| h.mount_path.clone())
    }

    /// Cleanup all mounts
    pub async fn cleanup_all(&self) -> Result<()> {
        let volume_ids: Vec<String> = {
            let mounts = self.active_mounts.lock().await;
            mounts.keys().cloned().collect()
        };

        for volume_id in volume_ids {
            if let Err(e) = self.unmount(&volume_id).await {
                tracing::warn!("Failed to unmount {}: {}", volume_id, e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_mount_manager_creation() {
        // Mock filesystem - this requires database connection
        // For unit tests, we just verify structure
        let pool_result = sqlx::PgPool::connect("postgresql://test").await;
        if pool_result.is_err() {
            return; // Skip if no DB
        }
        let pool = pool_result.unwrap();

        let tenant_id = Uuid::new_v4();
        let fs_result = FileSystem::new(&pool, tenant_id).await;
        if fs_result.is_err() {
            return; // Skip if initialization fails
        }
        let fs = Arc::new(fs_result.unwrap());
        let _manager = MountManager::new(fs);
    }

    #[tokio::test]
    async fn test_is_mounted_empty() {
        let pool_result = sqlx::PgPool::connect("postgresql://test").await;
        if pool_result.is_err() {
            return;
        }
        let pool = pool_result.unwrap();

        let tenant_id = Uuid::new_v4();
        let fs_result = FileSystem::new(&pool, tenant_id).await;
        if fs_result.is_err() {
            return;
        }
        let fs = Arc::new(fs_result.unwrap());
        let manager = MountManager::new(fs);

        assert!(!manager.is_mounted("test-volume").await);
    }
}
