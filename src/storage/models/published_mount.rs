use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Publish target type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "layer_id", rename_all = "snake_case")]
pub enum PublishTarget {
    /// Publish a specific snapshot (fixed content)
    Layer(Uuid),

    /// Publish working layer (real-time updates)
    WorkingLayer,
}

/// Publish scope (access control)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PublishScope {
    /// Accessible by all tenants
    Public,

    /// Accessible only by specific tenants
    AllowList { tenants: Vec<Uuid> },
}

/// Published mount record
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedMount {
    pub publish_id: Uuid,
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub publish_name: String,
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for publishing a mount
#[derive(Debug, Clone)]
pub struct PublishMountInput {
    pub mount_entry_id: Uuid,
    pub publish_name: String,
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
}

/// Input for updating publish info
#[derive(Debug, Clone, Default)]
pub struct UpdatePublishInput {
    pub description: Option<String>,
    pub scope: Option<PublishScope>,
}

/// Filter for querying published mounts
#[derive(Debug, Clone, Default)]
pub struct PublishedMountFilter {
    pub scope: Option<String>,         // "public" or "all"
    pub owner_tenant_id: Option<Uuid>, // Filter by owner
}

/// Resolved published mount (with actual layer_id for working_layer types)
#[derive(Debug, Clone)]
pub struct ResolvedPublished {
    pub mount_entry_id: Uuid,
    pub owner_tenant_id: Uuid,
    pub layer_id: Uuid,
    pub is_working_layer: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_target_layer_serialization() {
        let target = PublishTarget::Layer(Uuid::new_v4());
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("\"type\":\"layer\""));

        let deserialized: PublishTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, deserialized);
    }

    #[test]
    fn test_publish_target_working_layer_serialization() {
        let target = PublishTarget::WorkingLayer;
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("\"type\":\"working_layer\""));

        let deserialized: PublishTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, deserialized);
    }

    #[test]
    fn test_publish_scope_public_serialization() {
        let scope = PublishScope::Public;
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("\"type\":\"public\""));

        let deserialized: PublishScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn test_publish_scope_allow_list_serialization() {
        let tenant1 = Uuid::new_v4();
        let tenant2 = Uuid::new_v4();
        let scope = PublishScope::AllowList { tenants: vec![tenant1, tenant2] };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("\"type\":\"allow_list\""));
        assert!(json.contains("tenants"));

        let deserialized: PublishScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn test_publish_scope_allow_list_empty() {
        let scope = PublishScope::AllowList { tenants: vec![] };
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: PublishScope = serde_json::from_str(&json).unwrap();
        match deserialized {
            PublishScope::AllowList { tenants } => assert!(tenants.is_empty()),
            _ => panic!("Expected AllowList"),
        }
    }

    #[test]
    fn test_publish_mount_input_creation() {
        let input = PublishMountInput {
            mount_entry_id: Uuid::new_v4(),
            publish_name: "test-publish".to_string(),
            description: Some("Test description".to_string()),
            target: PublishTarget::WorkingLayer,
            scope: PublishScope::Public,
        };
        assert_eq!(input.publish_name, "test-publish");
        assert!(input.description.is_some());
    }

    #[test]
    fn test_update_publish_input_default() {
        let input = UpdatePublishInput::default();
        assert!(input.description.is_none());
        assert!(input.scope.is_none());
    }

    #[test]
    fn test_published_mount_filter_default() {
        let filter = PublishedMountFilter::default();
        assert!(filter.scope.is_none());
        assert!(filter.owner_tenant_id.is_none());
    }

    #[test]
    fn test_resolved_published_creation() {
        let resolved = ResolvedPublished {
            mount_entry_id: Uuid::new_v4(),
            owner_tenant_id: Uuid::new_v4(),
            layer_id: Uuid::new_v4(),
            is_working_layer: true,
        };
        assert!(resolved.is_working_layer);
    }
}
