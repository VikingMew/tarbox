# Docker Compose 使用指南

## 概述

本项目提供了 Docker Compose 配置，用于快速启动开发和测试环境。

## 服务说明

### 核心服务

1. **postgres** - PostgreSQL 16 数据库（生产/开发）
   - 端口: 5432
   - 用户: postgres
   - 密码: postgres
   - 数据库: tarbox
   - 数据持久化: postgres_data volume

2. **postgres-test** - PostgreSQL 16 测试数据库
   - 端口: 5433
   - 用户: postgres
   - 密码: postgres
   - 数据库: tarbox_test
   - 使用 tmpfs，数据不持久化（重启后清空）

3. **tarbox-cli** - Tarbox CLI 容器
   - 用于手动测试和交互式操作
   - 自动连接到 postgres 服务

### 可选服务（需要 --profile tools）

4. **pgadmin** - PostgreSQL Web 管理界面
   - 端口: 5050
   - 邮箱: admin@tarbox.local
   - 密码: admin

## 快速开始

### 1. 启动核心服务

```bash
# 启动 PostgreSQL 数据库
docker-compose up -d postgres postgres-test

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f postgres
```

### 2. 初始化数据库

```bash
# 使用本地 tarbox CLI 初始化
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
cargo run -- init

# 或使用容器内的 tarbox CLI
docker-compose run --rm tarbox-cli tarbox init
```

### 3. 运行测试

```bash
# 设置测试数据库 URL
export DATABASE_URL=postgres://postgres:postgres@localhost:5433/tarbox_test

# 运行集成测试
cargo test --test storage_integration_test

# 或在容器内运行
docker-compose run --rm tarbox-cli cargo test
```

### 4. 使用 CLI 工具

```bash
# 进入 tarbox-cli 容器
docker-compose exec tarbox-cli bash

# 在容器内使用 tarbox 命令
tarbox tenant create test-agent
tarbox --tenant test-agent mkdir /data
tarbox --tenant test-agent ls /
```

### 5. 启动 pgAdmin（可选）

```bash
# 使用 tools profile 启动 pgAdmin
docker-compose --profile tools up -d pgadmin

# 访问 http://localhost:5050
# 登录: admin@tarbox.local / admin

# 添加服务器连接:
#   Host: postgres
#   Port: 5432
#   Username: postgres
#   Password: postgres
```

## 常用命令

### 服务管理

```bash
# 启动所有核心服务
docker-compose up -d

# 启动所有服务（包括可选服务）
docker-compose --profile tools up -d

# 停止所有服务
docker-compose down

# 停止并删除数据卷
docker-compose down -v

# 重启特定服务
docker-compose restart postgres

# 查看服务日志
docker-compose logs -f postgres
docker-compose logs -f tarbox-cli
```

### 数据库操作

```bash
# 连接到 PostgreSQL
docker-compose exec postgres psql -U postgres -d tarbox

# 导出数据库
docker-compose exec postgres pg_dump -U postgres tarbox > backup.sql

# 导入数据库
cat backup.sql | docker-compose exec -T postgres psql -U postgres tarbox

# 重置数据库
docker-compose exec postgres psql -U postgres -c "DROP DATABASE IF EXISTS tarbox;"
docker-compose exec postgres psql -U postgres -c "CREATE DATABASE tarbox;"
```

### 开发工作流

```bash
# 1. 启动数据库
docker-compose up -d postgres postgres-test

# 2. 在本地开发
cargo build
cargo test

# 3. 使用容器测试
docker-compose build tarbox-cli
docker-compose run --rm tarbox-cli cargo test

# 4. 清理
docker-compose down
```

## 配置文件

### docker-compose.yml

主配置文件，定义所有服务。

### .env（需要创建）

从 `.env.example` 复制并修改：

```bash
cp .env.example .env
```

可配置的环境变量：
- `DATABASE_URL` - 主数据库连接 URL
- `TEST_DATABASE_URL` - 测试数据库连接 URL
- `RUST_LOG` - 日志级别
- `POSTGRES_*` - PostgreSQL 配置
- `PGADMIN_*` - pgAdmin 配置

## 数据持久化

### 开发环境

- **postgres** 使用 named volume `postgres_data`
- 数据在容器重启后保留
- 使用 `docker-compose down -v` 可以删除数据

### 测试环境

- **postgres-test** 使用 tmpfs
- 数据在内存中，容器停止后自动清空
- 每次测试都是干净的环境

## 网络配置

所有服务在 `tarbox-network` 网络中，可以通过服务名互相访问：

- `postgres:5432` - 主数据库
- `postgres-test:5432` - 测试数据库
- `tarbox-cli` - CLI 容器

## 健康检查

PostgreSQL 服务配置了健康检查：
- 每 5 秒检查一次
- 连续 5 次失败后标记为 unhealthy
- 依赖服务会等待健康检查通过

## 故障排查

### 端口冲突

如果端口已被占用：

```bash
# 查看端口占用
lsof -i :5432
lsof -i :5433
lsof -i :5050

# 修改 docker-compose.yml 中的端口映射
# 例如: "15432:5432" 而不是 "5432:5432"
```

### 数据库连接失败

```bash
# 检查服务状态
docker-compose ps

# 检查日志
docker-compose logs postgres

# 手动测试连接
docker-compose exec postgres psql -U postgres -d tarbox

# 重启服务
docker-compose restart postgres
```

### 权限问题

```bash
# 如果遇到 volume 权限问题
sudo chown -R $USER:$USER ./postgres_data

# 或删除 volume 重新创建
docker-compose down -v
docker-compose up -d
```

### 容器无法启动

```bash
# 查看详细日志
docker-compose logs --tail=100 postgres

# 重新构建
docker-compose build --no-cache tarbox-cli

# 删除所有容器和 volume 重新开始
docker-compose down -v
docker-compose up -d
```

## 生产环境注意事项

⚠️ **此 docker-compose 配置仅用于开发和测试，不适合生产环境！**

生产环境需要考虑：

1. **安全性**
   - 修改默认密码
   - 使用 secrets 管理敏感信息
   - 启用 SSL/TLS
   - 限制网络访问

2. **性能**
   - 调整 PostgreSQL 配置参数
   - 使用外部数据库服务
   - 配置连接池

3. **高可用**
   - PostgreSQL 主从复制
   - 负载均衡
   - 自动备份和恢复

4. **监控**
   - 添加 Prometheus + Grafana
   - 日志聚合
   - 告警配置

## 示例：完整工作流

```bash
# 1. 克隆项目
git clone <repo>
cd tarbox

# 2. 准备环境
cp .env.example .env

# 3. 启动服务
docker-compose up -d postgres postgres-test

# 4. 等待数据库就绪
docker-compose ps
# 等待 postgres 状态变为 healthy

# 5. 初始化数据库
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
cargo run -- init

# 6. 创建测试租户
cargo run -- tenant create test-agent

# 7. 运行文件操作
cargo run -- --tenant test-agent mkdir /data
cargo run -- --tenant test-agent ls /
cargo run -- --tenant test-agent touch /data/hello.txt
cargo run -- --tenant test-agent write /data/hello.txt "Hello, Tarbox!"
cargo run -- --tenant test-agent cat /data/hello.txt

# 8. 运行测试
export DATABASE_URL=postgres://postgres:postgres@localhost:5433/tarbox_test
cargo test

# 9. 清理
docker-compose down
```

## 参考资料

- [Docker Compose 文档](https://docs.docker.com/compose/)
- [PostgreSQL Docker 镜像](https://hub.docker.com/_/postgres)
- [pgAdmin 文档](https://www.pgadmin.org/docs/)
