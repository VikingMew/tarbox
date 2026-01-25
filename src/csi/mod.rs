pub mod controller;
pub mod identity;
pub mod metrics;
pub mod mount_manager;
pub mod node;
pub mod server;
pub mod snapshot;
pub mod tenant_mapping;

pub use controller::ControllerService;
pub use identity::IdentityService;
pub use mount_manager::MountManager;
pub use node::NodeService;
pub use server::CsiServer;
pub use snapshot::SnapshotManager;
pub use tenant_mapping::TenantMapper;

// Re-export generated proto types
pub mod proto {
    tonic::include_proto!("csi.v1");
}
