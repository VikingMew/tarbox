//! Layer management module for layered filesystem functionality.
//!
//! This module provides the core implementation of the layered filesystem,
//! including:
//! - Layer management (create, switch, delete, list)
//! - Write-time copy (COW) for files and directories
//! - Union view across layer chain
//! - File type detection (text vs binary)
//! - Text file diff and line-level COW
//! - Filesystem hooks (/.tarbox/)

mod cow;
mod detection;
mod hooks;
mod manager;
mod union_view;

pub use cow::{CowHandler, CowResult, TextChanges};
pub use detection::{DetectionConfig, FileTypeDetector, FileTypeInfo, LineEnding, TextEncoding};
pub use hooks::{HookError, HookFileAttr, HookResult, HooksHandler, TARBOX_HOOK_PATH};
pub use manager::{LayerManager, LayerManagerError};
pub use union_view::{DirectoryEntry, FileState, FileVersion, UnionView};
