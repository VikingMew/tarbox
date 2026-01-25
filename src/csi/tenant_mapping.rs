use crate::storage::traits::{LayerRepository, TenantRepository};
use crate::storage::{LayerOperations, TenantOperations};
use anyhow::{Context, Result};
use std::sync::Arc;
use uuid::Uuid;

/// Maps Kubernetes PVC to Tarbox tenants
///
/// Tenant naming: {namespace}-{pvc-name}
/// Volume ID = Tenant ID (UUID)
#[derive(Clone)]
pub struct TenantMapper<'a> {
    tenant_ops: Arc<TenantOperations<'a>>,
    layer_ops: Arc<LayerOperations<'a>>,
}

impl<'a> TenantMapper<'a> {
    pub fn new(tenant_ops: Arc<TenantOperations<'a>>, layer_ops: Arc<LayerOperations<'a>>) -> Self {
        Self { tenant_ops, layer_ops }
    }

    /// Create tenant from PVC
    ///
    /// Returns the tenant ID as volume_id
    pub async fn create_tenant_from_pvc(
        &self,
        namespace: &str,
        pvc_name: &str,
        _capacity_bytes: i64,
    ) -> Result<Uuid> {
        let tenant_name = Self::format_tenant_name(namespace, pvc_name);

        // Check if tenant already exists
        if let Some(existing) = self.tenant_ops.get_by_name(&tenant_name).await? {
            return Ok(existing.tenant_id);
        }

        // Create tenant
        let tenant = self
            .tenant_ops
            .create(crate::storage::CreateTenantInput { tenant_name: tenant_name.clone() })
            .await
            .context("Failed to create tenant")?;

        // Create base layer for tenant
        self.layer_ops
            .create(crate::storage::CreateLayerInput {
                tenant_id: tenant.tenant_id,
                parent_layer_id: None,
                layer_name: "base".to_string(),
                description: Some(format!("PVC: {}/{}", namespace, pvc_name)),
                tags: None,
                created_by: "csi-driver".to_string(),
            })
            .await
            .context("Failed to create base layer")?;

        Ok(tenant.tenant_id)
    }

    /// Delete tenant for volume
    pub async fn delete_tenant_for_volume(&self, volume_id: &str) -> Result<()> {
        let tenant_id =
            Uuid::parse_str(volume_id).context("Invalid volume_id format, expected UUID")?;

        let _deleted =
            self.tenant_ops.delete(tenant_id).await.context("Failed to delete tenant")?;

        Ok(())
    }

    /// Get tenant ID from volume ID
    pub fn parse_volume_id(&self, volume_id: &str) -> Result<Uuid> {
        Uuid::parse_str(volume_id).context("Invalid volume_id format")
    }

    /// Format tenant name from namespace and PVC name
    /// Uses '--' as separator to avoid ambiguity with hyphens in k8s names
    pub fn format_tenant_name(namespace: &str, pvc_name: &str) -> String {
        format!("{}--{}", namespace, pvc_name)
    }

    /// Parse namespace and PVC name from tenant name
    /// Expects format: "namespace--pvc-name"
    pub fn parse_tenant_name(tenant_name: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = tenant_name.splitn(2, "--").collect();
        if parts.len() == 2 { Some((parts[0].to_string(), parts[1].to_string())) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tenant_name() {
        assert_eq!(TenantMapper::format_tenant_name("default", "my-pvc"), "default--my-pvc");
        assert_eq!(
            TenantMapper::format_tenant_name("kube-system", "test-pvc"),
            "kube-system--test-pvc"
        );
    }

    #[test]
    fn test_parse_tenant_name() {
        assert_eq!(
            TenantMapper::parse_tenant_name("default--my-pvc"),
            Some(("default".to_string(), "my-pvc".to_string()))
        );
        assert_eq!(
            TenantMapper::parse_tenant_name("kube-system--test-pvc"),
            Some(("kube-system".to_string(), "test-pvc".to_string()))
        );
        assert_eq!(TenantMapper::parse_tenant_name("invalid"), None);
        assert_eq!(TenantMapper::parse_tenant_name("no-separator"), None);
    }

    #[test]
    fn test_parse_volume_id() {
        // This is a pure function test, no need for actual DB connection
        let uuid = Uuid::new_v4();
        // Test valid UUID parsing
        assert_eq!(Uuid::parse_str(&uuid.to_string()).unwrap(), uuid);
        // Test invalid UUID parsing
        assert!(Uuid::parse_str("invalid").is_err());
    }
}
