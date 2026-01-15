use anyhow::Result;
use async_trait::async_trait;

use crate::types::{InodeId, TenantId};

use super::models::{
    CreateBlockInput, CreateInodeInput, CreateTenantInput, DataBlock, Inode, Tenant,
    UpdateInodeInput,
};

#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn create(&self, input: CreateTenantInput) -> Result<Tenant>;
    async fn get_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>>;
    async fn get_by_name(&self, tenant_name: &str) -> Result<Option<Tenant>>;
    async fn list(&self) -> Result<Vec<Tenant>>;
    async fn delete(&self, tenant_id: TenantId) -> Result<bool>;
}

#[async_trait]
pub trait InodeRepository: Send + Sync {
    async fn create(&self, input: CreateInodeInput) -> Result<Inode>;
    async fn get(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Option<Inode>>;
    async fn get_by_parent_and_name(
        &self,
        tenant_id: TenantId,
        parent_id: InodeId,
        name: &str,
    ) -> Result<Option<Inode>>;
    async fn update(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        input: UpdateInodeInput,
    ) -> Result<Inode>;
    async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<bool>;
    async fn list_children(&self, tenant_id: TenantId, parent_id: InodeId) -> Result<Vec<Inode>>;
}

#[async_trait]
pub trait BlockRepository: Send + Sync {
    async fn create(&self, input: CreateBlockInput) -> Result<DataBlock>;
    async fn get(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        block_index: i32,
    ) -> Result<Option<DataBlock>>;
    async fn list(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Vec<DataBlock>>;
    async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<u64>;
}
