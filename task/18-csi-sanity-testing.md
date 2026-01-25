# Task 18: CSI Sanity 测试

## 状态

**✅ 已完成** (2026-01-25)

### 完成总结

- ✅ `tarbox csi` 子命令实现（支持 controller/node/all 三种模式）
- ✅ `CsiServer::serve_all()` 方法添加
- ✅ 本地测试脚本 `scripts/csi-sanity-test.sh`
- ✅ E2E workflow 集成 csi-sanity 测试
- ✅ 所有测试通过，fmt + clippy 检查通过

**使用方法**:
```bash
# 本地运行
./scripts/csi-sanity-test.sh

# 或手动运行
cargo build --release
./target/release/tarbox csi --mode=all --endpoint=unix:///tmp/csi.sock
```

## 目标

使用 Kubernetes 官方的 csi-sanity 工具测试 CSI 驱动合规性，无需真实 K8s 集群。

## 优先级

**P1 - 测试覆盖**

## 依赖

- Task 13: Kubernetes CSI Driver ✅

## 背景

csi-sanity 是 Kubernetes SIG-Storage 维护的 CSI 合规性测试工具，通过 Unix socket 直接调用 gRPC 接口测试，不需要 K8s 集群。

## 实现内容

### 1. 添加 `tarbox csi` 命令

启动独立的 CSI gRPC 服务器：

```bash
tarbox csi --mode=controller --endpoint=unix:///tmp/csi.sock
tarbox csi --mode=node --endpoint=unix:///tmp/csi.sock --node-id=test
tarbox csi --mode=all --endpoint=unix:///tmp/csi.sock --node-id=test
```

### 2. 集成到 E2E workflow

在 `.github/workflows/e2e.yml` 中添加 csi-sanity 测试 job。

### 3. 本地测试脚本

`scripts/csi-sanity-test.sh` - 一键运行 csi-sanity 测试。

## 测试范围

### 可以测试（无需 K8s）

- Identity Service（全部）
- Controller: CreateVolume, DeleteVolume, ListVolumes, Snapshots
- Node: GetInfo, GetCapabilities

### 需要跳过（无 kubelet 环境）

- NodePublishVolume 实际挂载
- NodeStageVolume staging
- Mount propagation

## 验收标准

- [ ] `tarbox csi` 命令可启动 CSI 服务器
- [ ] csi-sanity Identity 测试全部通过
- [ ] csi-sanity Controller 测试通过
- [ ] csi-sanity Node 基础测试通过
- [ ] GitHub Actions E2E 集成通过

## 时间估算

约 1 天

## 参考资料

- [csi-sanity](https://github.com/kubernetes-csi/csi-test)
- [CSI Spec](https://github.com/container-storage-interface/spec)
