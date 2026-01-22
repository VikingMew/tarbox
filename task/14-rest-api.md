# Task 14: REST API

## 状态

**📅 计划中**

## 目标

实现完整的 REST API 服务，基于 Axum 框架提供 HTTP 接口，供 WASI 模块、外部客户端和 Web Dashboard 访问。

**核心功能**：
- 文件系统操作 API
- 租户管理 API
- Layer 管理 API
- 审计查询 API
- 文本文件差异 API
- 系统管理 API

## 优先级

**P2 - 云原生集成**

## 依赖

- Task 05: FUSE 接口 ✅ (FilesystemInterface)
- Task 06: 数据库层高级 ✅
- Task 08: 分层文件系统 ✅

## 依赖的Spec

- **spec/06-api-design.md** - API 设计规范（核心）
- spec/14-filesystem-interface.md - 文件系统接口

## 实现内容

### 1. API 服务器框架 (`src/api/`)

- [ ] REST 服务器 (Axum)
- [ ] 路由定义
- [ ] 中间件（认证、日志、CORS）
- [ ] 错误处理
- [ ] 请求/响应模型

### 2. 文件系统 API

- [ ] GET /fs/stat
- [ ] GET /fs/list
- [ ] GET /fs/read
- [ ] POST /fs/write
- [ ] POST /fs/mkdir
- [ ] DELETE /fs/remove
- [ ] POST /fs/rename
- [ ] POST /fs/copy

### 3. Layer 管理 API

- [ ] GET /layers
- [ ] GET /layers/{id}
- [ ] POST /layers/checkpoint
- [ ] POST /layers/switch
- [ ] GET /layers/current
- [ ] GET /layers/{id}/changes
- [ ] POST /layers/squash
- [ ] DELETE /layers/{id}

### 4. 审计 API

- [ ] GET /audit/query
- [ ] GET /audit/stats
- [ ] GET /audit/user-activity

### 5. 文本文件 API

- [ ] GET /files/diff
- [ ] GET /files/history
- [ ] GET /files/content

### 6. 系统管理 API

- [ ] GET /system/status
- [ ] GET /system/metrics
- [ ] POST /system/cache/flush
- [ ] GET /health

## 验收标准

- [ ] 所有 API 端点实现
- [ ] 认证和授权工作
- [ ] 错误处理完整
- [ ] OpenAPI 文档生成
- [ ] 集成测试覆盖率 >80%

## 预计工作量

2-3 周

## 参考资料

- spec/06-api-design.md
- Axum 文档
