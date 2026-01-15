# Task 01: 项目初始化和基础设施搭建

## 目标

搭建项目基础结构，配置开发环境，建立代码规范和 CI/CD 流程。

## 优先级

**P0 - 最高优先级**

## 依赖

无

## 状态

**✅ 已完成** - 2026-01-15

## 子任务

### 1.1 项目结构创建

- [x] 创建标准的 Rust 项目目录结构
  - `src/main.rs` - 入口点
  - `src/lib.rs` - 库入口
  - `src/fuse/` - FUSE 接口模块
  - `src/fs/` - 文件系统核心模块
  - `src/storage/` - PostgreSQL 存储层
  - `src/audit/` - 审计系统
  - `src/layer/` - 分层文件系统
  - `src/cache/` - 缓存层
  - `src/api/` - REST 和 gRPC API
  - `src/k8s/` - Kubernetes CSI 驱动
  - `src/config/` - 配置管理
  - `src/types.rs` - 公共类型定义

- [x] 创建测试目录
  - `tests/` - 集成测试
  - `benches/` - 性能基准测试

- [x] 创建配置和部署目录
  - `deploy/kubernetes/` - K8s 部署文件
  - `examples/` - 示例配置

### 1.2 配置文件

- [x] 创建 `.gitignore`
  - Rust 编译产物
  - IDE 配置文件
  - 临时文件和日志
  - 数据库凭证

- [x] 创建 `.editorconfig`
  - 统一代码格式

- [x] 创建 `rustfmt.toml`
  - 代码格式化规则

- [x] 创建 `clippy.toml`
  - Clippy lint 规则

### 1.3 CI/CD 配置

- [x] GitHub Actions 工作流
  - `.github/workflows/ci.yml` - 持续集成
    - 编译检查
    - 运行测试
    - Clippy 检查
    - 格式检查
    - PostgreSQL 服务用于测试
  - `.github/workflows/release.yml` - 发布流程
    - 构建多架构 release 二进制
      - Linux: x86_64-gnu (Intel/AMD), aarch64-gnu (ARM64), x86_64-musl (静态链接)
      - macOS: aarch64-darwin (Apple Silicon)
    - 创建 GitHub release
    - 上传打包后的二进制文件
  - `.github/workflows/docker.yml` - Docker 镜像构建
    - 多架构镜像 (amd64, arm64)
    - 推送到 GitHub Container Registry
    - 标签策略：
      - 分支推送: `20260115123456-abc1234` (timestamp-commitid)
      - Tag 推送: `v1.2.3` (完整版本号，仅 v 开头的 tag)
      - 无 latest 标签，无版本缩减

- [x] 代码质量检查
  - 配置 `cargo deny` - 依赖检查
  - 配置 `cargo audit` - 安全漏洞检查

- [x] Docker 配置
  - `Dockerfile` - 多阶段构建
    - 依赖缓存优化
    - 最小化运行时镜像
    - 非 root 用户运行
    - 健康检查配置
  - `.dockerignore` - 排除不必要的文件

### 1.4 开发工具配置

- [x] 创建 `Makefile` 或 `justfile`
  - 常用命令快捷方式
  - `make test` - 运行测试
  - `make bench` - 运行基准测试
  - `make fmt` - 格式化代码
  - `make lint` - 运行 lint
  - `make build` - 构建项目
  - `make clean` - 清理产物

- [ ] VSCode 配置（可选）
  - `.vscode/settings.json` - 编辑器设置
  - `.vscode/launch.json` - 调试配置
  - `.vscode/extensions.json` - 推荐扩展

### 1.5 文档

- [x] 创建 `CONTRIBUTING.md`
  - 贡献指南
  - 开发流程
  - 代码规范

- [x] 创建 `CODE_OF_CONDUCT.md`
  - 行为准则

- [x] 创建 `LICENSE`
  - MIT 或 Apache-2.0

- [ ] 更新 `README.md`
  - 添加徽章（CI 状态、版本等）
  - 添加快速开始指南

### 1.6 基础类型定义

- [x] 在 `src/types.rs` 定义核心类型
  - InodeId
  - LayerId
  - TenantId
  - BlockId
  - 等常用类型别名

### 1.7 日志和追踪

- [x] 配置 tracing
  - 初始化 tracing subscriber
  - 配置日志级别
  - 配置日志格式（开发/生产）
  - 支持环境变量配置

- [x] 添加基础的日志宏使用示例

### 1.8 配置系统

- [x] 实现配置加载
  - 从文件加载（TOML）
  - 从环境变量覆盖
  - 配置验证
  - 默认配置

- [x] 定义配置结构
  - DatabaseConfig
  - FuseConfig
  - AuditConfig
  - CacheConfig
  - ApiConfig

## 验收标准

- [x] 项目可以成功编译（`cargo build`）
- [x] 所有测试通过（`cargo test`）
- [x] 代码格式正确（`cargo fmt --check`）
- [x] Clippy 无警告（`cargo clippy -- -D warnings`）
- [x] CI/CD 流程可以正常运行
- [x] 文档完整且可读

## 预估时间

2-3 天

## 项目准则

### 测试覆盖率要求

**所有代码必须保持测试覆盖率 > 80%**。这是项目级别的要求，以确保代码质量和可靠性。

- 为所有新函数编写单元测试
- 为完整工作流编写集成测试
- 测试边界情况和错误条件
- 使用 `cargo test` 运行测试
- 考虑使用 `cargo-tarpaulin` 或类似工具测量覆盖率

## 编程原则

本项目遵循 Linus Torvalds 和 John Carmack 的编程哲学：

### Linus Torvalds 原则

- **简单直接**：代码应该简单明了，避免过度抽象
- **实用主义**：解决实际问题，不追求完美主义
- **可读性优先**：代码是给人读的，其次才是给机器执行的
- **避免过度设计**：不为未来可能的需求设计，专注当前问题
- **Fail fast**：让错误尽早暴露，不要隐藏问题

### John Carmack 原则

- **功能性编程思维**：尽可能使用纯函数，减少可变状态
- **数据导向设计**：先考虑数据结构，代码围绕数据组织
- **性能意识**：了解底层实现，避免不必要的开销
- **小函数原则**：函数应该短小精悍，易于理解和测试
- **显式优于隐式**：明确表达意图，避免魔法和隐藏行为

### 具体实践

- 不定义复杂的错误类型系统，使用 `anyhow` 让错误直接抛出
- 避免过度抽象和 trait 设计，除非确实需要多态
- 优先使用简单的数据结构（struct、enum）
- 函数保持简短，单一职责
- 避免过早优化，但保持对性能的意识
- 代码注释说明"为什么"，而不是"是什么"

## 注意事项

- 确保 Rust 版本为 1.92+
- 确保 Edition 2024
- 所有配置文件使用 UTF-8 编码
- 遵循 Rust 社区最佳实践

## 后续任务

完成后可以开始：
- Task 02: 数据库模块实现
- Task 03: 基础文件系统实现
