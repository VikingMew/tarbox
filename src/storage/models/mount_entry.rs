use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Mount source type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MountSource {
    /// Host filesystem directory or file
    Host { path: PathBuf },

    /// Layer from another mount point
    Layer { source_mount_id: Uuid, layer_id: Option<Uuid>, subpath: Option<PathBuf> },

    /// Published layer (referenced by name)
    Published { publish_name: String, subpath: Option<PathBuf> },

    /// Current tenant's working layer for this mount point
    WorkingLayer,
}

/// Mount mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    /// Read-only
    #[serde(rename = "ro")]
    #[default]
    ReadOnly,

    /// Read-write (only valid for Host or WorkingLayer)
    #[serde(rename = "rw")]
    ReadWrite,

    /// Copy-on-write (read from source, write to working layer)
    #[serde(rename = "cow")]
    CopyOnWrite,
}

impl MountMode {
    pub fn parse_mode(s: &str) -> Option<Self> {
        match s {
            "ro" => Some(Self::ReadOnly),
            "rw" => Some(Self::ReadWrite),
            "cow" => Some(Self::CopyOnWrite),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "ro",
            Self::ReadWrite => "rw",
            Self::CopyOnWrite => "cow",
        }
    }
}

/// Mount entry
#[derive(Debug, Clone, PartialEq)]
pub struct MountEntry {
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,
    pub enabled: bool,
    pub current_layer_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a mount entry
#[derive(Debug, Clone)]
pub struct CreateMountEntry {
    pub name: String,
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,
    pub metadata: Option<serde_json::Value>,
}

/// Input for updating a mount entry
#[derive(Debug, Clone, Default)]
pub struct UpdateMountEntry {
    pub mode: Option<MountMode>,
    pub enabled: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_mode_from_str() {
        assert_eq!(MountMode::parse_mode("ro"), Some(MountMode::ReadOnly));
        assert_eq!(MountMode::parse_mode("rw"), Some(MountMode::ReadWrite));
        assert_eq!(MountMode::parse_mode("cow"), Some(MountMode::CopyOnWrite));
        assert_eq!(MountMode::parse_mode("invalid"), None);
    }

    #[test]
    fn test_mount_mode_as_str() {
        assert_eq!(MountMode::ReadOnly.as_str(), "ro");
        assert_eq!(MountMode::ReadWrite.as_str(), "rw");
        assert_eq!(MountMode::CopyOnWrite.as_str(), "cow");
    }

    #[test]
    fn test_mount_mode_default() {
        assert_eq!(MountMode::default(), MountMode::ReadOnly);
    }

    #[test]
    fn test_mount_source_host_serialization() {
        let source = MountSource::Host { path: PathBuf::from("/usr") };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"host\""));
        assert!(json.contains("\"/usr\""));

        let deserialized: MountSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_mount_source_layer_serialization() {
        let source = MountSource::Layer {
            source_mount_id: Uuid::new_v4(),
            layer_id: Some(Uuid::new_v4()),
            subpath: Some(PathBuf::from("/models")),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"layer\""));

        let deserialized: MountSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_mount_source_published_serialization() {
        let source =
            MountSource::Published { publish_name: "bert-base-v1".to_string(), subpath: None };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"published\""));
        assert!(json.contains("bert-base-v1"));

        let deserialized: MountSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_mount_source_working_layer_serialization() {
        let source = MountSource::WorkingLayer;
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"working_layer\""));

        let deserialized: MountSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_mount_mode_serialization() {
        assert_eq!(serde_json::to_string(&MountMode::ReadOnly).unwrap(), "\"ro\"");
        assert_eq!(serde_json::to_string(&MountMode::ReadWrite).unwrap(), "\"rw\"");
        assert_eq!(serde_json::to_string(&MountMode::CopyOnWrite).unwrap(), "\"cow\"");
    }
}
