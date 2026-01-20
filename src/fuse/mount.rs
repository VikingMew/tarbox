// FUSE mount management
//
// Provides functions to mount and unmount Tarbox filesystems via FUSE.

use super::{FuseAdapter, TarboxBackend};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

/// Mount options for FUSE filesystem
#[derive(Debug, Clone)]
pub struct MountOptions {
    /// Allow other users to access the filesystem
    pub allow_other: bool,

    /// Allow root to access the filesystem
    pub allow_root: bool,

    /// Mount as read-only
    pub read_only: bool,

    /// Filesystem name (for mtab)
    pub fsname: Option<String>,

    /// Auto-unmount on process exit
    pub auto_unmount: bool,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            allow_other: false,
            allow_root: false,
            read_only: false,
            fsname: Some("tarbox".to_string()),
            auto_unmount: true,
        }
    }
}

impl MountOptions {
    /// Convert to fuser mount options
    fn to_fuser_options(&self) -> Vec<fuser::MountOption> {
        let mut options = Vec::new();

        if self.allow_other {
            options.push(fuser::MountOption::AllowOther);
        }

        if self.allow_root {
            options.push(fuser::MountOption::AllowRoot);
        }

        if self.read_only {
            options.push(fuser::MountOption::RO);
        }

        if let Some(ref fsname) = self.fsname {
            options.push(fuser::MountOption::FSName(fsname.clone()));
        }

        if self.auto_unmount {
            options.push(fuser::MountOption::AutoUnmount);
        }

        options
    }
}

/// Mount a Tarbox filesystem via FUSE
///
/// # Arguments
/// * `backend` - Pre-created TarboxBackend instance
/// * `mountpoint` - Directory to mount at
/// * `options` - Mount options
///
/// # Returns
/// A session handle that keeps the filesystem mounted until dropped
///
/// # Note
/// The FUSE adapter creates its own dedicated tokio runtime internally.
/// This allows the mount function to be called from any context (sync or async)
/// without risk of deadlocks.
pub fn mount(
    backend: Arc<TarboxBackend>,
    mountpoint: impl AsRef<Path>,
    options: MountOptions,
) -> Result<fuser::BackgroundSession> {
    let mountpoint = mountpoint.as_ref();

    // Validate mountpoint exists and is a directory
    if !mountpoint.exists() {
        anyhow::bail!("Mount point does not exist: {}", mountpoint.display());
    }

    if !mountpoint.is_dir() {
        anyhow::bail!("Mount point is not a directory: {}", mountpoint.display());
    }

    // Create FUSE adapter with its own dedicated runtime
    // The adapter manages its own runtime to avoid deadlocks when called from async context
    let adapter = FuseAdapter::new(backend);

    // Convert mount options
    let fuser_options = options.to_fuser_options();

    tracing::info!("Mounting Tarbox filesystem at {}", mountpoint.display());

    // Mount filesystem in background
    let session = fuser::spawn_mount2(adapter, mountpoint, &fuser_options)
        .context("Failed to mount filesystem")?;

    tracing::info!("Filesystem mounted successfully");

    Ok(session)
}

/// Unmount a FUSE filesystem
///
/// Note: This is automatically handled when the BackgroundSession is dropped,
/// but this function can be used for explicit unmounting.
pub fn unmount(mountpoint: impl AsRef<Path>) -> Result<()> {
    let mountpoint = mountpoint.as_ref();

    tracing::info!("Unmounting filesystem at {}", mountpoint.display());

    // On Linux, we can use fusermount -u
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let output = Command::new("fusermount")
            .arg("-u")
            .arg(mountpoint)
            .output()
            .context("Failed to execute fusermount")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to unmount: {}", stderr);
        }
    }

    // On macOS, we use umount
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let output =
            Command::new("umount").arg(mountpoint).output().context("Failed to execute umount")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to unmount: {}", stderr);
        }
    }

    tracing::info!("Filesystem unmounted successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_options_default() {
        let options = MountOptions::default();
        assert!(!options.allow_other);
        assert!(!options.allow_root);
        assert!(!options.read_only);
        assert_eq!(options.fsname, Some("tarbox".to_string()));
        assert!(options.auto_unmount);
    }

    #[test]
    fn test_mount_options_to_fuser() {
        let options = MountOptions {
            allow_other: true,
            allow_root: true,
            read_only: true,
            fsname: Some("test".to_string()),
            auto_unmount: false,
        };

        let fuser_options = options.to_fuser_options();

        // Should contain the options
        assert!(fuser_options.contains(&fuser::MountOption::AllowOther));
        assert!(fuser_options.contains(&fuser::MountOption::AllowRoot));
        assert!(fuser_options.contains(&fuser::MountOption::RO));
        assert!(fuser_options.contains(&fuser::MountOption::FSName("test".to_string())));
    }

    #[test]
    fn test_mount_options_builder() {
        let options = MountOptions { allow_other: true, read_only: true, ..Default::default() };

        assert!(options.allow_other);
        assert!(options.read_only);
        assert!(!options.allow_root);
    }
}
