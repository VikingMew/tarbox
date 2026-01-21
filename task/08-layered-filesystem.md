# Task 08: 分层文件系统实现

## 目标

实现类似 Docker 镜像层的分层文件系统，包括：
- **写时复制（COW）**: 文件和目录的增量修改
- **检查点管理**: 创建、命名、切换文件系统快照
- **层间切换**: 在历史层间自由切换
- **联合视图**: 多层文件的合并读取
- **虚拟文件系统钩子**: `/.tarbox/layers/` 控制接口

## 优先级

**P1 - 高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建 ✅
- Task 02: 数据库存储层 MVP ✅
- Task 03: 文件系统核心 MVP ✅
- Task 05: FUSE 接口 ✅
- Task 06: 数据库层高级功能（需要 layers、layer_entries 表）✅

## 依赖的Spec

- **spec/04-layered-filesystem.md** - 分层架构、COW 策略、层切换逻辑（核心）
- **spec/08-filesystem-hooks.md** - `/.tarbox/` 虚拟目录设计和命令接口（核心）
- **spec/10-text-file-optimization.md** - 文本文件的行级 COW 和 diff 算法
- spec/03-audit-system.md - 层操作的审计日志记录
- spec/14-filesystem-interface.md - FilesystemInterface 统一抽象

## 完成状态

**状态: 核心功能已完成 ✅**

### 已实现的模块

#### 1. 层管理核心模块 (`src/layer/mod.rs`) ✅
- `manager.rs` - 层管理器
- `cow.rs` - 写时复制处理
- `union_view.rs` - 联合视图
- `detection.rs` - 文件类型检测
- `hooks.rs` - 文件系统钩子

#### 2. 文件类型检测 (`src/layer/detection.rs`) ✅
- [x] UTF-8 编码验证
- [x] ASCII 文本检测
- [x] 行结构分析（LF, CRLF, CR, Mixed）
- [x] 行数统计
- [x] 最大行长度检测
- [x] 二进制文件检测（null 字节、大文件）
- [x] 可配置的检测阈值

#### 3. 层管理器 (`src/layer/manager.rs`) ✅
- [x] `initialize_base_layer()` - 初始化基础层
- [x] `get_current_layer()` - 获取当前层
- [x] `get_current_layer_id()` - 获取当前层 ID
- [x] `list_layers()` - 列出所有层
- [x] `create_checkpoint()` - 创建检查点
- [x] `create_checkpoint_with_confirm()` - 创建检查点（支持历史层确认）
- [x] `switch_to_layer()` - 切换到指定层
- [x] `delete_layer()` - 删除层
- [x] `get_layer()` - 获取层详情
- [x] `get_layer_entries()` - 获取层条目
- [x] `is_historical_layer()` - 检测是否为历史层
- [x] `get_future_layers()` - 获取未来层列表
- [x] 历史层检测和确认机制

#### 4. 写时复制 (`src/layer/cow.rs`) ✅
- [x] `CowHandler` 结构体
- [x] `write_file()` - 写入文件（自动检测类型）
- [x] `write_binary_file()` - 写入二进制文件（块级 COW）
- [x] `write_text_file()` - 写入文本文件（行级 COW）
- [x] `read_text_file()` - 读取文本文件内容
- [x] 使用 `similar` crate 计算文本 diff
- [x] `TextChanges` 统计（新增、删除、修改行数）
- [x] `CowResult` 返回操作结果

#### 5. 联合视图 (`src/layer/union_view.rs`) ✅
- [x] `UnionView` 结构体
- [x] `load()` - 加载层链
- [x] `lookup_file()` - 查找文件（沿层链向上）
- [x] `list_directory()` - 列出目录（合并多层）
- [x] `get_file_history()` - 获取文件历史
- [x] `find_file_layer()` - 查找文件所在层
- [x] `FileState` 枚举（Exists, Deleted, NotFound）
- [x] `FileVersion` 结构体
- [x] `DirectoryEntry` 结构体

#### 6. 文件系统钩子 (`src/layer/hooks.rs`) ✅
- [x] `HooksHandler` 结构体
- [x] `is_hook_path()` - 检测钩子路径
- [x] `handle_read()` - 处理读取操作
- [x] `handle_write()` - 处理写入操作
- [x] `get_attr()` - 获取文件属性
- [x] `read_dir()` - 列出目录

虚拟路径实现：
- [x] `/.tarbox/` - 根目录
- [x] `/.tarbox/layers/` - 层目录
- [x] `/.tarbox/layers/current` - 当前层信息（JSON）
- [x] `/.tarbox/layers/list` - 层列表（JSON）
- [x] `/.tarbox/layers/new` - 创建新层
- [x] `/.tarbox/layers/switch` - 切换层
- [x] `/.tarbox/layers/drop` - 删除层
- [x] `/.tarbox/layers/tree` - 层树视图
- [x] `/.tarbox/layers/diff` - 当前层差异
- [x] `/.tarbox/snapshots/` - 快照目录
- [x] `/.tarbox/stats/` - 统计目录
- [x] `/.tarbox/stats/usage` - 使用统计

#### 7. FUSE 集成 (`src/fuse/backend.rs`) ✅
- [x] `is_hook_path()` - 钩子路径检测
- [x] `hook_attr_to_file_attr()` - 属性转换
- [x] `hook_error_to_fs_error()` - 错误转换
- [x] `hooks_handler()` - 获取钩子处理器
- [x] `read_file()` - 支持钩子读取
- [x] `write_file()` - 支持钩子写入
- [x] `get_attr()` - 支持钩子属性
- [x] `read_dir()` - 支持钩子目录列表（含 `.tarbox` 虚拟条目）
- [x] 所有修改操作对钩子路径返回 PermissionDenied

### 测试状态

- [x] 单元测试：134 个测试全部通过
- [x] 集成测试：全部通过
- [x] fmt 检查通过
- [x] clippy 检查通过

## 子任务

### 5.1 层管理基础 ✅

- [x] 实现层数据结构
- [x] 实现基础层（Base Layer）
- [x] 实现当前层管理

### 5.2 检查点（Checkpoint）操作 ✅

- [x] 实现创建检查点
- [x] 实现检查点命名
- [x] 实现检查点描述

### 5.3 层间切换 ✅

- [x] 实现切换到历史层
- [x] 实现非破坏性切换
- [x] 实现在历史层创建新层（需确认）

### 5.4 文件类型检测与管理 ✅

- [x] 实现文本/二进制自动检测
- [x] 实现文本文件阈值管理
- [x] 可配置的检测参数

### 5.5 写时复制（COW）实现 ✅

- [x] 实现二进制文件 COW（块级）
- [x] 实现文本文件 COW（行级 diff）
- [x] 使用 similar crate 计算 diff

### 5.6 联合视图（Union View）✅

- [x] 实现文件查找
- [x] 实现目录合并
- [x] 实现层链遍历

### 5.7 层操作 ✅

- [x] 实现列出所有层
- [x] 实现查看层详情
- [x] 实现查看层差异

### 5.8 层删除 ✅

- [x] 实现删除层
- [x] 验证是否可删除

### 5.9 层合并（Squash）

- [ ] 实现层压缩（未来扩展）
- [ ] 实现智能合并（未来扩展）

### 5.10 文件历史 ✅

- [x] 实现文件历史查询
- [ ] 实现文件版本对比（未来扩展）
- [ ] 实现文件恢复（未来扩展）

### 5.11 层数据管理 ✅

- [x] 实现层条目管理
- [x] 使用现有的引用计数机制
- [x] 层统计信息

### 5.12 文件系统 Hook 实现 ✅

- [x] 实现虚拟目录 `/.tarbox/`
- [x] 实现层控制接口
- [x] 实现命令解析
- [x] 实现 snapshots 视图

### 5.13 性能优化

- [ ] 层链缓存（未来优化）
- [ ] 增量查询（未来优化）
- [ ] 延迟加载（未来优化）

### 5.14 一致性保证 ✅

- [x] 使用数据库事务
- [x] 基本的数据完整性检查
- [ ] 高级并发控制（未来增强）

### 5.15 测试 ✅

- [x] 单元测试
- [x] 基本集成测试
- [ ] 性能测试（未来添加）
- [ ] 压力测试（未来添加）

## 验收标准

- [x] 可以创建检查点
- [x] 可以切换到历史层
- [x] COW 正确工作（文本和二进制）
- [x] 联合视图正确显示文件
- [x] 文件历史可以查询
- [x] 文件系统 Hook 正常工作
- [x] 层删除正确
- [x] 所有测试通过

## 文件清单

新增文件：
- `src/layer/mod.rs` - 模块入口
- `src/layer/detection.rs` - 文件类型检测
- `src/layer/manager.rs` - 层管理器
- `src/layer/cow.rs` - 写时复制
- `src/layer/union_view.rs` - 联合视图
- `src/layer/hooks.rs` - 文件系统钩子

修改文件：
- `src/lib.rs` - 添加 layer 模块导出
- `src/fuse/backend.rs` - 集成钩子处理

## 技术实现说明

### 文件类型检测
使用 `FileTypeDetector` 类自动检测文件类型：
- UTF-8 验证确定是否为文本
- 检测 null 字节判断二进制
- 分析行结构（LF/CRLF/CR）
- 可配置最大文件大小和行长度阈值

### 写时复制机制
- **二进制文件**: 块级 COW，使用内容哈希去重
- **文本文件**: 行级 COW，使用 `similar` crate 计算 diff
- 自动检测文件类型并选择合适的 COW 策略

### 联合视图
从当前层开始向上遍历层链：
- 文件查找：返回最近层的版本
- 目录合并：合并所有层的目录项，处理删除标记
- 文件历史：收集文件在各层的版本信息

### 钩子实现
虚拟文件系统，不存储在数据库：
- 在 FUSE 层拦截 `/.tarbox/` 路径
- 读取操作返回动态生成的内容
- 写入操作触发层管理命令

## 后续任务

可以继续的工作：
- Task 07: 高级文件系统功能（ACL、符号链接等）
- 层合并（Squash）功能
- 性能优化和缓存
- 更多的测试覆盖
