use crate::csi::proto::{
    GetPluginCapabilitiesRequest, GetPluginCapabilitiesResponse, GetPluginInfoRequest,
    GetPluginInfoResponse, PluginCapability, ProbeRequest, ProbeResponse,
    identity_server::Identity,
};
use tonic::{Request, Response, Status};

const PLUGIN_NAME: &str = "tarbox.csi.io";
const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identity Service implementation
///
/// Provides plugin information and capabilities to Kubernetes.
#[derive(Debug, Clone)]
pub struct IdentityService {
    name: String,
    version: String,
}

impl IdentityService {
    pub fn new() -> Self {
        Self { name: PLUGIN_NAME.to_string(), version: PLUGIN_VERSION.to_string() }
    }
}

impl Default for IdentityService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl Identity for IdentityService {
    async fn get_plugin_info(
        &self,
        _request: Request<GetPluginInfoRequest>,
    ) -> Result<Response<GetPluginInfoResponse>, Status> {
        Ok(Response::new(GetPluginInfoResponse {
            name: self.name.clone(),
            vendor_version: self.version.clone(),
            manifest: Default::default(),
        }))
    }

    async fn get_plugin_capabilities(
        &self,
        _request: Request<GetPluginCapabilitiesRequest>,
    ) -> Result<Response<GetPluginCapabilitiesResponse>, Status> {
        use crate::csi::proto::plugin_capability::{Service, service::Type};

        let capabilities = vec![
            PluginCapability {
                r#type: Some(crate::csi::proto::plugin_capability::Type::Service(Service {
                    r#type: Type::ControllerService as i32,
                })),
            },
            PluginCapability {
                r#type: Some(crate::csi::proto::plugin_capability::Type::Service(Service {
                    r#type: Type::VolumeAccessibilityConstraints as i32,
                })),
            },
        ];

        Ok(Response::new(GetPluginCapabilitiesResponse { capabilities }))
    }

    async fn probe(
        &self,
        _request: Request<ProbeRequest>,
    ) -> Result<Response<ProbeResponse>, Status> {
        Ok(Response::new(ProbeResponse { ready: Some(true) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_plugin_info() {
        let service = IdentityService::new();
        let request = Request::new(GetPluginInfoRequest {});

        let response = service.get_plugin_info(request).await.unwrap();
        let info = response.into_inner();

        assert_eq!(info.name, PLUGIN_NAME);
        assert_eq!(info.vendor_version, PLUGIN_VERSION);
    }

    #[tokio::test]
    async fn test_get_plugin_capabilities() {
        let service = IdentityService::new();
        let request = Request::new(GetPluginCapabilitiesRequest {});

        let response = service.get_plugin_capabilities(request).await.unwrap();
        let caps = response.into_inner();

        assert_eq!(caps.capabilities.len(), 2);
    }

    #[tokio::test]
    async fn test_probe() {
        let service = IdentityService::new();
        let request = Request::new(ProbeRequest {});

        let response = service.probe(request).await.unwrap();
        let probe = response.into_inner();

        assert_eq!(probe.ready, Some(true));
    }
}
