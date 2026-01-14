# Tarbox

一个基于 PostgreSQL 的分布式文件系统，专为 AI Agent 和云原生环境设计。

## 概述

Tarbox 是一个高性能的文件系统实现，将 PostgreSQL 作为存储后端，为 AI Agent 提供可靠、可审计的文件存储解决方案。通过 FUSE 接口提供完整的 POSIX 兼容性，支持 Kubernetes 持久卷（PV）挂载。

## 核心特性

### 1. PostgreSQL 存储后端
- 利用 PostgreSQL 的 ACID 特性保证数据一致性
- 支持分布式部署和高可用配置
- 元数据与数据分离存储，优化查询性能
- 支持大对象（Large Object）存储大文件

### 2. POSIX 兼容性
- 完整的 POSIX 文件系统接口实现
- 支持标准文件操作（read, write, open, close, mkdir, etc.）
- 支持文件权限和属性管理
- 符号链接和硬链接支持

### 3. 文件系统审计
- 完整的操作日志记录
- 文件访问追踪和审计
- 版本历史管理
- 合规性报告支持

### 4. 分层文件系统
- 类似 Docker 镜像层的版本化设计
- 写时复制（Copy-on-Write）技术
- 支持创建检查点（Checkpoint）
- 快速回滚和层间切换
- 通过文件系统 Hook 控制（如 `echo "checkpoint" > /.tarbox/layers/new`）

### 5. FUSE 接口
- 基于 FUSE（Filesystem in Userspace）实现
- 无需内核模块，易于部署
- 跨平台支持（Linux, macOS, FreeBSD）
- 高性能的用户态文件系统

### 6. 文本文件优化
- 针对 CSV、Markdown、YAML、HTML、代码等文本文件的智能支持
- 行级差异存储（类似 Git），层间只存储变化的行
- 跨文件、跨层的内容去重
- 高效的文件版本对比（`tarbox diff`）
- 完整的文件历史追踪
- 对应用层完全透明，保持标准 POSIX 语义

### 7. Kubernetes 集成
- 原生 Kubernetes PV/PVC 支持
- CSI（Container Storage Interface）驱动
- 动态卷供应
- 多租户隔离
- 快照和备份支持

## 架构

```
┌─────────────────────────────────────────────┐
│         应用层 / AI Agent                    │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│           FUSE Interface                     │
│  (POSIX-compliant File Operations)          │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│         Tarbox Core Engine                   │
│  ┌──────────────────────────────────────┐   │
│  │  文件系统层                           │   │
│  │  - Inode 管理                        │   │
│  │  - 目录树                            │   │
│  │  - 权限控制                          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  审计层                              │   │
│  │  - 操作日志                          │   │
│  │  - 版本控制                          │   │
│  │  - 访问追踪                          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  分层文件系统管理                     │   │
│  │  - Layer 管理（创建/切换/删除）      │   │
│  │  - 写时复制（COW）                   │   │
│  │  - 检查点（Checkpoint）              │   │
│  │  - 层合并和分支                      │   │
│  └──────────────────────────────────────┘   │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│        PostgreSQL Storage Backend            │
│  - 元数据表 (Metadata Tables)               │
│  - 数据块存储 (Data Blocks)                 │
│  - 审计日志表 (Audit Logs)                  │
│  - 层管理表 (Layer Tables)                  │
└──────────────────────────────────────────────┘
```

## 快速开始

### 前置要求

- Rust 1.92+ (Edition 2024)
- PostgreSQL 14+
- FUSE 库（Linux: libfuse3, macOS: macFUSE）

### 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/tarbox.git
cd tarbox

# 构建项目
cargo build --release

# 安装
cargo install --path .
```

### 配置

创建配置文件 `tarbox.toml`:

```toml
[database]
host = "localhost"
port = 5432
database = "tarbox"
user = "tarbox_user"
password = "your_password"
pool_size = 20

[filesystem]
mount_point = "/mnt/tarbox"
block_size = 4096
max_file_size = "10GB"

[audit]
enabled = true
log_level = "info"
retention_days = 90

[layer]
auto_checkpoint = false
checkpoint_interval = "1h"
```

### 运行

```bash
# 初始化数据库
tarbox init --config tarbox.toml

# 挂载文件系统
tarbox mount --config tarbox.toml

# 卸载文件系统
tarbox umount /mnt/tarbox
```

## Kubernetes 部署

### 安装 CSI 驱动

```bash
kubectl apply -f deploy/kubernetes/csi-driver.yaml
```

### 创建 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox
provisioner: tarbox.csi.io
parameters:
  database: "tarbox"
  auditEnabled: "true"
  autoCheckpoint: "false"
reclaimPolicy: Retain
allowVolumeExpansion: true
```

### 创建 PVC

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
```

### 在 Pod 中使用

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ai-agent
spec:
  containers:
  - name: agent
    image: your-agent-image:latest
    volumeMounts:
    - name: storage
      mountPath: /data
  volumes:
  - name: storage
    persistentVolumeClaim:
      claimName: agent-storage
```

## 性能优化

### 数据库优化

```sql
-- 为元数据表创建索引
CREATE INDEX idx_inode_parent ON inodes(parent_id);
CREATE INDEX idx_inode_name ON inodes(name);
CREATE INDEX idx_blocks_inode ON blocks(inode_id);

-- 启用并行查询
SET max_parallel_workers_per_gather = 4;
```

### 缓存配置

```toml
[cache]
metadata_cache_size = "1GB"
block_cache_size = "4GB"
cache_policy = "lru"
```

## 审计和监控

### 查询审计日志

```sql
-- 查看最近的文件操作
SELECT * FROM audit_logs 
WHERE created_at > NOW() - INTERVAL '1 day'
ORDER BY created_at DESC;

-- 查看特定文件的访问历史
SELECT * FROM audit_logs 
WHERE inode_id = 12345
ORDER BY created_at DESC;

-- 查看文本文件的修改统计
SELECT 
    path,
    SUM((metadata->'text_changes'->>'lines_added')::int) as total_added,
    SUM((metadata->'text_changes'->>'lines_deleted')::int) as total_deleted
FROM audit_logs
WHERE metadata->'text_changes'->>'is_text_file' = 'true'
  AND created_at > NOW() - INTERVAL '7 days'
GROUP BY path
ORDER BY total_added + total_deleted DESC;
```

### 文件历史和差异

```bash
# 查看文件在不同层的历史
tarbox history /data/config.yaml

# 输出：
# Layer: checkpoint-3 (2026-01-15 10:30:00)
#   Size: 1.25KB, Lines: 103 (+5 -2 ~3)
# Layer: checkpoint-2 (2026-01-14 15:20:00)
#   Size: 1.20KB, Lines: 100 (+2 -1 ~1)

# 对比两个层的文件差异
tarbox diff checkpoint-1 checkpoint-2 /data/config.yaml

# 输出类似 git diff：
# --- /data/config.yaml (checkpoint-1)
# +++ /data/config.yaml (checkpoint-2)
# @@ -48,7 +48,8 @@
#  database:
#    host: localhost
# -  port: 5432
# +  port: 5433
# +  pool_size: 20
```

### Prometheus 指标

Tarbox 导出以下 Prometheus 指标：

- `tarbox_operations_total` - 文件系统操作总数
- `tarbox_operation_duration_seconds` - 操作延迟
- `tarbox_cache_hit_ratio` - 缓存命中率
- `tarbox_storage_usage_bytes` - 存储使用量
- `tarbox_layer_data_bytes` - 各层数据大小

## 开发

### 项目结构

```
tarbox/
├── src/
│   ├── main.rs              # 入口点
│   ├── fuse/                # FUSE 接口实现
│   ├── fs/                  # 文件系统核心
│   ├── storage/             # PostgreSQL 存储层
│   ├── audit/               # 审计系统
│   ├── layer/               # 分层文件系统管理
│   ├── cache/               # 缓存层
│   └── k8s/                 # Kubernetes CSI 驱动
├── spec/                    # 架构设计文档
├── task/                    # 开发任务计划
├── tests/                   # 集成测试
├── benchmarks/              # 性能基准测试
└── deploy/                  # 部署配置
    └── kubernetes/          # K8s 部署文件
```

### 运行测试

```bash
# 运行单元测试
cargo test

# 运行集成测试
cargo test --test '*' -- --test-threads=1

# 运行基准测试
cargo bench
```

## 路线图

- [ ] 基础文件系统实现
- [ ] PostgreSQL 存储后端
- [ ] FUSE 接口
- [ ] 审计系统
- [ ] 分层文件系统（Layer/Checkpoint）
- [ ] Kubernetes CSI 驱动
- [ ] 快照和备份
- [ ] 多租户支持
- [ ] 分布式部署

## 许可证

MIT OR Apache-2.0

## 贡献

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解详情。

## 联系方式

- 问题追踪: [GitHub Issues](https://github.com/yourusername/tarbox/issues)
- 文档: [https://tarbox.io/docs](https://tarbox.io/docs)
- 讨论: [GitHub Discussions](https://github.com/yourusername/tarbox/discussions)
