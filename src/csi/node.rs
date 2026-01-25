use crate::csi::proto::{
    NodeExpandVolumeRequest, NodeExpandVolumeResponse, NodeGetCapabilitiesRequest,
    NodeGetCapabilitiesResponse, NodeGetInfoRequest, NodeGetInfoResponse,
    NodeGetVolumeStatsRequest, NodeGetVolumeStatsResponse, NodePublishVolumeRequest,
    NodePublishVolumeResponse, NodeStageVolumeRequest, NodeStageVolumeResponse,
    NodeUnpublishVolumeRequest, NodeUnpublishVolumeResponse, NodeUnstageVolumeRequest,
    NodeUnstageVolumeResponse, node_server::Node,
};
use crate::csi::{MountManager, TenantMapper};
use std::path::PathBuf;
use std::sync::Arc;
use tonic::{Request, Response, Status};

const NODE_ID: &str = "tarbox-node";

/// Node Service implementation
///
/// Handles node-specific operations: mount/unmount, stats
#[derive(Clone)]
pub struct NodeService {
    tenant_mapper: Arc<TenantMapper<'static>>,
    mount_manager: Arc<MountManager<'static>>,
    node_id: String,
}

impl NodeService {
    pub fn new(
        tenant_mapper: Arc<TenantMapper<'static>>,
        mount_manager: Arc<MountManager<'static>>,
    ) -> Self {
        Self::with_node_id(tenant_mapper, mount_manager, NODE_ID.to_string())
    }

    pub fn with_node_id(
        tenant_mapper: Arc<TenantMapper<'static>>,
        mount_manager: Arc<MountManager<'static>>,
        node_id: String,
    ) -> Self {
        Self { tenant_mapper, mount_manager, node_id }
    }
}

#[tonic::async_trait]
impl Node for NodeService {
    async fn node_stage_volume(
        &self,
        request: Request<NodeStageVolumeRequest>,
    ) -> Result<Response<NodeStageVolumeResponse>, Status> {
        let req = request.into_inner();

        // Parse volume ID (tenant ID)
        let tenant_id = self
            .tenant_mapper
            .parse_volume_id(&req.volume_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid volume_id: {}", e)))?;

        // Determine read-only mode
        let read_only = req
            .volume_capability
            .as_ref()
            .and_then(|cap| cap.access_mode.as_ref())
            .map(|mode| {
                mode.mode == crate::csi::proto::volume_capability::access_mode::Mode::SingleNodeReaderOnly as i32
                    || mode.mode == crate::csi::proto::volume_capability::access_mode::Mode::MultiNodeReaderOnly as i32
            })
            .unwrap_or(false);

        // Mount at staging path
        let staging_path = PathBuf::from(req.staging_target_path);

        self.mount_manager
            .mount(&req.volume_id, tenant_id, staging_path, read_only)
            .await
            .map_err(|e| Status::internal(format!("Failed to mount volume: {}", e)))?;

        Ok(Response::new(NodeStageVolumeResponse {}))
    }

    async fn node_unstage_volume(
        &self,
        request: Request<NodeUnstageVolumeRequest>,
    ) -> Result<Response<NodeUnstageVolumeResponse>, Status> {
        let req = request.into_inner();

        self.mount_manager
            .unmount(&req.volume_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to unmount volume: {}", e)))?;

        Ok(Response::new(NodeUnstageVolumeResponse {}))
    }

    async fn node_publish_volume(
        &self,
        request: Request<NodePublishVolumeRequest>,
    ) -> Result<Response<NodePublishVolumeResponse>, Status> {
        let req = request.into_inner();

        // Bind mount from staging to target
        let staging_path = PathBuf::from(&req.staging_target_path);
        let target_path = PathBuf::from(&req.target_path);

        // Create target directory
        tokio::fs::create_dir_all(&target_path)
            .await
            .map_err(|e| Status::internal(format!("Failed to create target directory: {}", e)))?;

        // Bind mount
        let read_only = req.readonly;
        let mut mount_opts = vec!["bind".to_string()];
        if read_only {
            mount_opts.push("ro".to_string());
        }

        let status = std::process::Command::new("mount")
            .arg("--bind")
            .arg(&staging_path)
            .arg(&target_path)
            .status()
            .map_err(|e| Status::internal(format!("Failed to bind mount: {}", e)))?;

        if !status.success() {
            return Err(Status::internal(format!("Mount command failed with status: {}", status)));
        }

        Ok(Response::new(NodePublishVolumeResponse {}))
    }

    async fn node_unpublish_volume(
        &self,
        request: Request<NodeUnpublishVolumeRequest>,
    ) -> Result<Response<NodeUnpublishVolumeResponse>, Status> {
        let req = request.into_inner();

        let target_path = PathBuf::from(&req.target_path);

        // Unmount bind mount
        let status = std::process::Command::new("umount")
            .arg(&target_path)
            .status()
            .map_err(|e| Status::internal(format!("Failed to unmount: {}", e)))?;

        if !status.success() {
            // If already unmounted, that's okay
            tracing::warn!("Unmount failed, volume may already be unmounted");
        }

        Ok(Response::new(NodeUnpublishVolumeResponse {}))
    }

    async fn node_get_volume_stats(
        &self,
        request: Request<NodeGetVolumeStatsRequest>,
    ) -> Result<Response<NodeGetVolumeStatsResponse>, Status> {
        let req = request.into_inner();

        // Get mount path
        let volume_path = PathBuf::from(&req.volume_path);

        // Get filesystem stats using statvfs
        let stats = nix::sys::statvfs::statvfs(&volume_path)
            .map_err(|e| Status::internal(format!("Failed to get volume stats: {}", e)))?;

        let block_size = stats.block_size() as i64;
        let total_bytes = stats.blocks() as i64 * block_size;
        let available_bytes = stats.blocks_available() as i64 * block_size;
        let used_bytes = total_bytes - available_bytes;

        let total_inodes = stats.files() as i64;
        let available_inodes = stats.files_free() as i64;
        let used_inodes = total_inodes - available_inodes;

        Ok(Response::new(NodeGetVolumeStatsResponse {
            usage: vec![
                crate::csi::proto::VolumeUsage {
                    total: total_bytes,
                    available: available_bytes,
                    used: used_bytes,
                    unit: crate::csi::proto::volume_usage::Unit::Bytes as i32,
                },
                crate::csi::proto::VolumeUsage {
                    total: total_inodes,
                    available: available_inodes,
                    used: used_inodes,
                    unit: crate::csi::proto::volume_usage::Unit::Inodes as i32,
                },
            ],
            volume_condition: None,
        }))
    }

    async fn node_expand_volume(
        &self,
        _request: Request<NodeExpandVolumeRequest>,
    ) -> Result<Response<NodeExpandVolumeResponse>, Status> {
        // Tarbox handles expansion at controller level
        Err(Status::unimplemented("Node expansion not needed for Tarbox"))
    }

    async fn node_get_capabilities(
        &self,
        _request: Request<NodeGetCapabilitiesRequest>,
    ) -> Result<Response<NodeGetCapabilitiesResponse>, Status> {
        use crate::csi::proto::node_service_capability::{Rpc, rpc::Type};

        let capabilities = vec![Type::StageUnstageVolume, Type::GetVolumeStats]
            .into_iter()
            .map(|t| crate::csi::proto::NodeServiceCapability {
                r#type: Some(crate::csi::proto::node_service_capability::Type::Rpc(Rpc {
                    r#type: t as i32,
                })),
            })
            .collect();

        Ok(Response::new(NodeGetCapabilitiesResponse { capabilities }))
    }

    async fn node_get_info(
        &self,
        _request: Request<NodeGetInfoRequest>,
    ) -> Result<Response<NodeGetInfoResponse>, Status> {
        Ok(Response::new(NodeGetInfoResponse {
            node_id: self.node_id.clone(),
            max_volumes_per_node: 0, // No limit
            accessible_topology: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_constant() {
        // Verify NODE_ID is properly defined
        assert!(!NODE_ID.is_empty());
        assert_eq!(NODE_ID, "tarbox-node");
    }

    // Integration tests with actual DB connections will be in tests/ directory
    // using mockall to avoid lifetime issues with 'static requirements
}
