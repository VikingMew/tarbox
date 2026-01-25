use crate::csi::proto::{
    ControllerExpandVolumeRequest, ControllerExpandVolumeResponse,
    ControllerGetCapabilitiesRequest, ControllerGetCapabilitiesResponse, CreateSnapshotRequest,
    CreateSnapshotResponse, CreateVolumeRequest, CreateVolumeResponse, DeleteSnapshotRequest,
    DeleteSnapshotResponse, DeleteVolumeRequest, DeleteVolumeResponse, ListSnapshotsRequest,
    ListSnapshotsResponse, ListVolumesRequest, ListVolumesResponse, Snapshot,
    ValidateVolumeCapabilitiesRequest, ValidateVolumeCapabilitiesResponse, Volume,
    controller_server::Controller,
};
use crate::csi::{SnapshotManager, TenantMapper};
use crate::storage::TenantOperations;
use crate::storage::traits::TenantRepository;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

/// Controller Service implementation
///
/// Handles volume lifecycle: create, delete, snapshot, clone, expand
#[derive(Clone)]
pub struct ControllerService {
    tenant_mapper: Arc<TenantMapper<'static>>,
    snapshot_manager: Arc<SnapshotManager<'static>>,
    tenant_ops: Arc<TenantOperations<'static>>,
}

impl ControllerService {
    pub fn new(
        tenant_mapper: Arc<TenantMapper<'static>>,
        snapshot_manager: Arc<SnapshotManager<'static>>,
        tenant_ops: Arc<TenantOperations<'static>>,
    ) -> Self {
        Self { tenant_mapper, snapshot_manager, tenant_ops }
    }

    fn extract_pvc_info(
        parameters: &std::collections::HashMap<String, String>,
    ) -> (String, String) {
        let namespace = parameters
            .get("csi.storage.k8s.io/pvc/namespace")
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        let pvc_name = parameters
            .get("csi.storage.k8s.io/pvc/name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        (namespace, pvc_name)
    }
}

#[tonic::async_trait]
impl Controller for ControllerService {
    async fn create_volume(
        &self,
        request: Request<CreateVolumeRequest>,
    ) -> Result<Response<CreateVolumeResponse>, Status> {
        let req = request.into_inner();

        // Extract PVC information
        let (namespace, pvc_name) = Self::extract_pvc_info(&req.parameters);

        // Get capacity (default to 1GB if not specified)
        let capacity_bytes = req
            .capacity_range
            .as_ref()
            .and_then(|r| r.required_bytes.into())
            .unwrap_or(1024 * 1024 * 1024); // 1GB default

        // Create tenant
        let tenant_id = self
            .tenant_mapper
            .create_tenant_from_pvc(&namespace, &pvc_name, capacity_bytes)
            .await
            .map_err(|e| Status::internal(format!("Failed to create tenant: {}", e)))?;

        // Create volume response
        let volume = Volume {
            volume_id: tenant_id.to_string(),
            capacity_bytes,
            volume_context: req.parameters,
            content_source: None,
            accessible_topology: vec![],
        };

        Ok(Response::new(CreateVolumeResponse { volume: Some(volume) }))
    }

    async fn delete_volume(
        &self,
        request: Request<DeleteVolumeRequest>,
    ) -> Result<Response<DeleteVolumeResponse>, Status> {
        let req = request.into_inner();

        self.tenant_mapper
            .delete_tenant_for_volume(&req.volume_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to delete tenant: {}", e)))?;

        Ok(Response::new(DeleteVolumeResponse {}))
    }

    async fn controller_publish_volume(
        &self,
        _request: Request<crate::csi::proto::ControllerPublishVolumeRequest>,
    ) -> Result<Response<crate::csi::proto::ControllerPublishVolumeResponse>, Status> {
        // Tarbox doesn't require attach/detach
        Ok(Response::new(crate::csi::proto::ControllerPublishVolumeResponse {
            publish_context: Default::default(),
        }))
    }

    async fn controller_unpublish_volume(
        &self,
        _request: Request<crate::csi::proto::ControllerUnpublishVolumeRequest>,
    ) -> Result<Response<crate::csi::proto::ControllerUnpublishVolumeResponse>, Status> {
        // Tarbox doesn't require attach/detach
        Ok(Response::new(crate::csi::proto::ControllerUnpublishVolumeResponse {}))
    }

    async fn validate_volume_capabilities(
        &self,
        request: Request<ValidateVolumeCapabilitiesRequest>,
    ) -> Result<Response<ValidateVolumeCapabilitiesResponse>, Status> {
        let req = request.into_inner();

        // Parse volume ID
        let tenant_id = self
            .tenant_mapper
            .parse_volume_id(&req.volume_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid volume_id: {}", e)))?;

        // Check if tenant exists
        let _tenant = self
            .tenant_ops
            .get_by_id(tenant_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get tenant: {}", e)))?
            .ok_or_else(|| Status::not_found("Volume not found"))?;

        // Validate capabilities (we support all POSIX operations)
        Ok(Response::new(ValidateVolumeCapabilitiesResponse {
            confirmed: Some(crate::csi::proto::validate_volume_capabilities_response::Confirmed {
                volume_context: Default::default(),
                volume_capabilities: req.volume_capabilities,
                parameters: Default::default(),
                mutable_parameters: Default::default(),
            }),
            message: String::new(),
        }))
    }

    async fn list_volumes(
        &self,
        request: Request<ListVolumesRequest>,
    ) -> Result<Response<ListVolumesResponse>, Status> {
        let _req = request.into_inner();

        // List all tenants
        let tenants = self
            .tenant_ops
            .list()
            .await
            .map_err(|e| Status::internal(format!("Failed to list tenants: {}", e)))?;

        let entries: Vec<_> = tenants
            .into_iter()
            .map(|tenant| crate::csi::proto::list_volumes_response::Entry {
                volume: Some(Volume {
                    volume_id: tenant.tenant_id.to_string(),
                    capacity_bytes: 0, // Capacity is managed at PVC level, not stored in tenant
                    volume_context: Default::default(),
                    content_source: None,
                    accessible_topology: vec![],
                }),
                status: None,
            })
            .collect();

        Ok(Response::new(ListVolumesResponse { entries, next_token: String::new() }))
    }

    async fn get_capacity(
        &self,
        _request: Request<crate::csi::proto::GetCapacityRequest>,
    ) -> Result<Response<crate::csi::proto::GetCapacityResponse>, Status> {
        // Return unlimited capacity
        Ok(Response::new(crate::csi::proto::GetCapacityResponse {
            available_capacity: i64::MAX,
            maximum_volume_size: None,
            minimum_volume_size: None,
        }))
    }

    async fn controller_get_capabilities(
        &self,
        _request: Request<ControllerGetCapabilitiesRequest>,
    ) -> Result<Response<ControllerGetCapabilitiesResponse>, Status> {
        use crate::csi::proto::controller_service_capability::{Rpc, rpc::Type};

        let capabilities = vec![
            Type::CreateDeleteVolume,
            Type::ListVolumes,
            Type::GetCapacity,
            Type::CreateDeleteSnapshot,
            Type::ListSnapshots,
            Type::CloneVolume,
            Type::ExpandVolume,
        ]
        .into_iter()
        .map(|t| crate::csi::proto::ControllerServiceCapability {
            r#type: Some(crate::csi::proto::controller_service_capability::Type::Rpc(Rpc {
                r#type: t as i32,
            })),
        })
        .collect();

        Ok(Response::new(ControllerGetCapabilitiesResponse { capabilities }))
    }

    async fn create_snapshot(
        &self,
        request: Request<CreateSnapshotRequest>,
    ) -> Result<Response<CreateSnapshotResponse>, Status> {
        let req = request.into_inner();

        // Parse volume ID
        let tenant_id = self
            .tenant_mapper
            .parse_volume_id(&req.source_volume_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid volume_id: {}", e)))?;

        // Create snapshot
        let layer = self
            .snapshot_manager
            .create_snapshot(tenant_id, &req.name)
            .await
            .map_err(|e| Status::internal(format!("Failed to create snapshot: {}", e)))?;

        let snapshot = Snapshot {
            snapshot_id: layer.layer_id.to_string(),
            source_volume_id: req.source_volume_id,
            creation_time: Some(prost_types::Timestamp {
                seconds: layer.created_at.timestamp(),
                nanos: 0,
            }),
            ready_to_use: true,
            size_bytes: 0, // TODO: calculate actual size
            group_snapshot_id: String::new(),
        };

        Ok(Response::new(CreateSnapshotResponse { snapshot: Some(snapshot) }))
    }

    async fn delete_snapshot(
        &self,
        request: Request<DeleteSnapshotRequest>,
    ) -> Result<Response<DeleteSnapshotResponse>, Status> {
        let req = request.into_inner();

        // Parse snapshot ID
        let _snapshot_id = Uuid::parse_str(&req.snapshot_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid snapshot_id: {}", e)))?;

        // We need tenant_id, which we don't have in the request
        // For now, we'll try to find it from the snapshot
        // In production, we'd need to store snapshot metadata separately

        // TODO: Implement proper snapshot deletion with tenant lookup

        Ok(Response::new(DeleteSnapshotResponse {}))
    }

    async fn list_snapshots(
        &self,
        request: Request<ListSnapshotsRequest>,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let req = request.into_inner();

        // If source_volume_id is provided, list snapshots for that volume
        if !req.source_volume_id.is_empty() {
            let tenant_id = self
                .tenant_mapper
                .parse_volume_id(&req.source_volume_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid volume_id: {}", e)))?;

            let layers = self
                .snapshot_manager
                .list_snapshots(tenant_id)
                .await
                .map_err(|e| Status::internal(format!("Failed to list snapshots: {}", e)))?;

            let entries: Vec<_> = layers
                .into_iter()
                .map(|layer| crate::csi::proto::list_snapshots_response::Entry {
                    snapshot: Some(Snapshot {
                        snapshot_id: layer.layer_id.to_string(),
                        source_volume_id: req.source_volume_id.clone(),
                        creation_time: Some(prost_types::Timestamp {
                            seconds: layer.created_at.timestamp(),
                            nanos: 0,
                        }),
                        ready_to_use: true,
                        size_bytes: 0,
                        group_snapshot_id: String::new(),
                    }),
                })
                .collect();

            return Ok(Response::new(ListSnapshotsResponse { entries, next_token: String::new() }));
        }

        // List all snapshots (not implemented for now)
        Ok(Response::new(ListSnapshotsResponse { entries: vec![], next_token: String::new() }))
    }

    async fn controller_expand_volume(
        &self,
        request: Request<ControllerExpandVolumeRequest>,
    ) -> Result<Response<ControllerExpandVolumeResponse>, Status> {
        let req = request.into_inner();

        // Parse volume ID
        let tenant_id = self
            .tenant_mapper
            .parse_volume_id(&req.volume_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid volume_id: {}", e)))?;

        // Get new capacity
        let new_capacity = req
            .capacity_range
            .as_ref()
            .and_then(|r| r.required_bytes.into())
            .ok_or_else(|| Status::invalid_argument("Missing required_bytes"))?;

        // Update tenant quota
        // TODO: Implement quota update in TenantOperations when quota tracking is added
        // For now, we accept any expansion request since Tarbox doesn't enforce hard quotas
        let _ = tenant_id; // Silencing unused warning until quota is implemented

        Ok(Response::new(ControllerExpandVolumeResponse {
            capacity_bytes: new_capacity,
            node_expansion_required: false,
        }))
    }

    async fn controller_get_volume(
        &self,
        _request: Request<crate::csi::proto::ControllerGetVolumeRequest>,
    ) -> Result<Response<crate::csi::proto::ControllerGetVolumeResponse>, Status> {
        Err(Status::unimplemented("ControllerGetVolume not implemented"))
    }

    async fn controller_modify_volume(
        &self,
        _request: Request<crate::csi::proto::ControllerModifyVolumeRequest>,
    ) -> Result<Response<crate::csi::proto::ControllerModifyVolumeResponse>, Status> {
        Err(Status::unimplemented("ControllerModifyVolume not implemented"))
    }

    async fn get_snapshot(
        &self,
        request: Request<crate::csi::proto::GetSnapshotRequest>,
    ) -> Result<Response<crate::csi::proto::GetSnapshotResponse>, Status> {
        let req = request.into_inner();

        // Parse snapshot ID
        let _snapshot_id = uuid::Uuid::parse_str(&req.snapshot_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid snapshot_id: {}", e)))?;

        // We need tenant_id but don't have it in the request
        // This is a limitation - in production we'd need snapshot metadata table
        Err(Status::unimplemented("GetSnapshot requires tenant context"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pvc_info() {
        let mut params = std::collections::HashMap::new();
        params.insert("csi.storage.k8s.io/pvc/namespace".to_string(), "default".to_string());
        params.insert("csi.storage.k8s.io/pvc/name".to_string(), "my-pvc".to_string());

        let (ns, name) = ControllerService::extract_pvc_info(&params);
        assert_eq!(ns, "default");
        assert_eq!(name, "my-pvc");
    }

    #[test]
    fn test_extract_pvc_info_defaults() {
        let params = std::collections::HashMap::new();
        let (ns, name) = ControllerService::extract_pvc_info(&params);
        assert_eq!(ns, "default");
        assert_eq!(name, "unknown");
    }
}
