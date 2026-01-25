use crate::csi::proto::{
    controller_server::ControllerServer, identity_server::IdentityServer, node_server::NodeServer,
};
use crate::csi::{ControllerService, IdentityService, NodeService};
use anyhow::{Context, Result};
use tonic::transport::Server;

/// CSI gRPC server
#[allow(dead_code)] // Fields used in serve methods
pub struct CsiServer {
    identity: IdentityService,
    controller: Option<ControllerService>,
    node: Option<NodeService>,
}

impl CsiServer {
    pub fn new(
        identity: IdentityService,
        controller: Option<ControllerService>,
        node: Option<NodeService>,
    ) -> Self {
        Self { identity, controller, node }
    }

    /// Serve controller service
    pub async fn serve_controller(
        identity: IdentityService,
        controller: ControllerService,
        addr: String,
    ) -> Result<()> {
        let addr_parsed =
            addr.strip_prefix("unix://").context("Address must start with unix://")?;

        // Remove existing socket if it exists
        if std::path::Path::new(addr_parsed).exists() {
            std::fs::remove_file(addr_parsed).context("Failed to remove existing socket")?;
        }

        // Create parent directory
        if let Some(parent) = std::path::Path::new(addr_parsed).parent() {
            std::fs::create_dir_all(parent).context("Failed to create socket directory")?;
        }

        // Create UDS listener
        let uds =
            tokio::net::UnixListener::bind(addr_parsed).context("Failed to bind Unix socket")?;
        let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

        tracing::info!("CSI Controller listening on {}", addr);

        Server::builder()
            .add_service(IdentityServer::new(identity))
            .add_service(ControllerServer::new(controller))
            .serve_with_incoming(uds_stream)
            .await
            .context("gRPC server error")
    }

    /// Serve node service
    pub async fn serve_node(
        identity: IdentityService,
        node: NodeService,
        addr: String,
    ) -> Result<()> {
        let addr_parsed =
            addr.strip_prefix("unix://").context("Address must start with unix://")?;

        // Remove existing socket if it exists
        if std::path::Path::new(addr_parsed).exists() {
            std::fs::remove_file(addr_parsed).context("Failed to remove existing socket")?;
        }

        // Create parent directory
        if let Some(parent) = std::path::Path::new(addr_parsed).parent() {
            std::fs::create_dir_all(parent).context("Failed to create socket directory")?;
        }

        // Create UDS listener
        let uds =
            tokio::net::UnixListener::bind(addr_parsed).context("Failed to bind Unix socket")?;
        let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

        tracing::info!("CSI Node listening on {}", addr);

        Server::builder()
            .add_service(IdentityServer::new(identity))
            .add_service(NodeServer::new(node))
            .serve_with_incoming(uds_stream)
            .await
            .context("gRPC server error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csi_server_creation() {
        let identity = IdentityService::new();
        let server = CsiServer::new(identity, None, None);
        assert!(server.controller.is_none());
        assert!(server.node.is_none());
    }
}
