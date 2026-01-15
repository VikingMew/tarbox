# Task 02: 数据库存储层（MVP）

## 目标

实现最小可用的 PostgreSQL 存储层，支持基础的租户管理和文件元数据存储。

## 优先级

**P0 - 最高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建

## 子任务

### 2.1 数据库连接管理

- [ ] 实现连接池管理
  - 使用 sqlx 的连接池
  - 基础配置（max_connections, timeout）
  - 实现健康检查

- [ ] 实现数据库初始化
  - 检查数据库连接
  - 验证 PostgreSQL 版本（>= 14）

### 2.2 最小 Schema

- [ ] 创建 tenants 表
  - tenant_id (UUID, PK)
  - tenant_name (VARCHAR, UNIQUE)
  - root_inode_id (BIGINT)
  - created_at, updated_at

- [ ] 创建 inodes 表
  - inode_id (BIGSERIAL)
  - tenant_id (UUID, FK)
  - parent_id (BIGINT, nullable)
  - name (VARCHAR(255))
  - inode_type (VARCHAR: file, dir, symlink)
  - mode, uid, gid, size
  - atime, mtime, ctime
  - PRIMARY KEY (tenant_id, inode_id)

- [ ] 创建 data_blocks 表
  - block_id (UUID, PK)
  - tenant_id (UUID, FK)
  - inode_id (BIGINT)
  - block_index (INTEGER)
  - data (BYTEA)
  - size (INTEGER)
  - content_hash (VARCHAR, 用于去重)
  - created_at

- [ ] 创建基础索引
  - idx_inodes_parent: (tenant_id, parent_id, name)
  - idx_blocks_inode: (tenant_id, inode_id, block_index)
  - idx_blocks_hash: (content_hash)

### 2.3 租户操作

- [ ] 实现 create_tenant()
  - 插入 tenant 记录
  - 创建根目录 inode (inode_id=1, type=dir, name="/")
  - 返回 tenant_id

- [ ] 实现 get_tenant()
  - 按 tenant_id 查询
  - 按 tenant_name 查询

- [ ] 实现 delete_tenant()
  - 标记删除（先不实际删除数据）

### 2.4 Inode 操作

- [ ] 实现 create_inode()
  - 插入 inode 记录
  - 自动设置时间戳
  - 返回 inode_id

- [ ] 实现 get_inode()
  - 按 (tenant_id, inode_id) 查询
  - 返回完整 inode 信息

- [ ] 实现 get_inode_by_parent_and_name()
  - 路径解析用：查找子节点
  - WHERE tenant_id = ? AND parent_id = ? AND name = ?

- [ ] 实现 update_inode()
  - 更新 size, mtime 等字段
  - 用于写入后更新元数据

- [ ] 实现 delete_inode()
  - 删除 inode 记录
  - 简单实现，不处理级联

- [ ] 实现 list_children()
  - 列出目录的所有子项
  - WHERE tenant_id = ? AND parent_id = ?

### 2.5 数据块操作

- [ ] 实现 create_block()
  - 插入数据块
  - 计算 content_hash (blake3)
  - 返回 block_id

- [ ] 实现 get_block()
  - 按 (tenant_id, inode_id, block_index) 查询
  - 返回数据内容

- [ ] 实现 list_blocks()
  - 列出文件的所有块
  - 按 block_index 排序

- [ ] 实现 delete_blocks()
  - 删除 inode 的所有数据块

### 2.6 事务支持

- [ ] 实现 begin_transaction()
  - 开启数据库事务

- [ ] 实现 commit_transaction()
  - 提交事务

- [ ] 实现 rollback_transaction()
  - 回滚事务

### 2.7 测试

- [ ] 租户 CRUD 测试
- [ ] Inode CRUD 测试
- [ ] 数据块 CRUD 测试
- [ ] 事务测试

## 验收标准

- [ ] 可以创建和查询租户
- [ ] 可以创建和查询 inode
- [ ] 可以存储和读取数据块
- [ ] 事务正确工作
- [ ] 所有测试通过

## 预估时间

3-4 天

## 注意事项

- 所有操作必须包含 tenant_id
- 使用参数化查询防止 SQL 注入
- 数据块大小限制为 4KB
- content_hash 用于简单去重

## 后续任务

完成后可以开始：
- Task 03: 基础文件系统实现（MVP）
- Task 06: 数据库层完善（审计、分层、文本优化）
