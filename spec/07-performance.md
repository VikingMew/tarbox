# 性能优化设计

## 概述

性能是 Tarbox 的关键指标，本文档定义性能目标、优化策略和测试方法。

## 性能目标

### 吞吐量目标

```
小文件操作（< 4KB）：
- 顺序读：100k IOPS
- 顺序写：50k IOPS
- 随机读：80k IOPS
- 随机写：40k IOPS

大文件操作（> 1MB）：
- 顺序读：2 GB/s
- 顺序写：1 GB/s
- 随机读：500 MB/s
- 随机写：300 MB/s

元数据操作：
- stat：100k ops/s
- readdir：50k ops/s
- create/delete：30k ops/s
```

### 延迟目标

```
P50（中位数）：
- 元数据操作：< 1ms
- 小文件读写：< 2ms
- 大文件读写：< 10ms

P99（99分位）：
- 元数据操作：< 5ms
- 小文件读写：< 10ms
- 大文件读写：< 50ms

P999（99.9分位）：
- 元数据操作：< 20ms
- 小文件读写：< 50ms
- 大文件读写：< 200ms
```

### 并发性能

```
并发连接：
- 单节点：1000 并发连接
- 集群：10000 并发连接

并发操作：
- 读操作：完全并行
- 写操作：文件级并行，块级串行
- 元数据操作：乐观锁 + 重试
```

## 缓存策略

### 多级缓存架构

```
L1 - 内存缓存（进程内）：
- 容量：可配置（默认 2GB）
- 淘汰：LRU
- 命中率目标：> 90%

L2 - 本地磁盘缓存（可选）：
- 容量：可配置（默认 20GB）
- 用于：节点本地缓存
- 适用场景：K8s DaemonSet

L3 - 分布式缓存（未来）：
- Redis/Memcached
- 跨节点共享
```

### 元数据缓存

```rust
// Inode 缓存
Cache<InodeId, InodeData> {
    max_size: 100_000,  // 最多缓存 10 万个 inode
    ttl: None,          // 永不过期（写入时更新）
    eviction: LRU,
}

// 路径缓存
Cache<PathBuf, InodeId> {
    max_size: 200_000,  // 最多缓存 20 万条路径
    ttl: 60s,           // 60 秒过期
    eviction: LRU,
}

// 目录缓存
Cache<InodeId, Vec<DirEntry>> {
    max_size: 10_000,   // 最多缓存 1 万个目录
    ttl: 30s,           // 30 秒过期
    eviction: LRU,
}

// 层链缓存
Cache<LayerId, Vec<LayerId>> {
    max_size: 1_000,    // 最多缓存 1000 条层链
    ttl: None,          // 永不过期（层关系不变）
    eviction: None,     // 不淘汰
}
```

### 数据块缓存

```rust
// 块缓存
Cache<(InodeId, BlockIndex), BlockData> {
    max_size_bytes: 4 * 1024 * 1024 * 1024,  // 4GB
    eviction: LRU,
    
    // 预读策略
    readahead: {
        // 检测顺序访问
        sequential_threshold: 3,  // 连续 3 次视为顺序
        readahead_size: 1MB,      // 每次预读 1MB
        max_readahead: 4MB,       // 最大预读 4MB
    }
}
```

### 缓存一致性

```
写穿（Write-Through）：
- 写操作同步更新缓存和数据库
- 保证强一致性
- 适用于元数据

写回（Write-Back）：
- 写操作仅更新缓存
- 异步刷新到数据库
- 性能更高但有丢失风险
- 适用于数据块（可配置）

缓存失效：
1. 主动失效：
   - 写入时更新缓存
   - 删除时清除缓存
   
2. TTL 过期：
   - 定时检查过期项
   - 懒惰删除

3. 容量淘汰：
   - LRU 算法
   - 达到容量上限时淘汰
```

### 缓存预热

```rust
// 启动时预热
async fn warmup_cache() {
    // 1. 加载常用路径
    let common_paths = ["/", "/data", "/workspace"];
    for path in common_paths {
        let _ = resolve_path(path).await;
    }
    
    // 2. 加载活跃层的元数据
    let active_layer = get_active_layer().await;
    let _ = load_layer_metadata(active_layer).await;
    
    // 3. 加载最近访问的文件
    let recent_files = query_recent_files(limit: 1000).await;
    for file in recent_files {
        let _ = get_inode(file.inode_id).await;
    }
}
```

## 数据库优化

### 连接池配置

```toml
[database.pool]
# 连接数
min_idle = 10          # 最小空闲连接
max_size = 50          # 最大连接数
acquire_timeout = 30s  # 获取连接超时

# 连接生命周期
max_lifetime = 30m     # 连接最长存活时间
idle_timeout = 10m     # 空闲连接超时

# 连接测试
test_on_checkout = true
test_query = "SELECT 1"
```

### 查询优化

```sql
-- 1. 索引优化
-- 确保所有外键都有索引
CREATE INDEX idx_inodes_parent ON inodes(parent_id);
CREATE INDEX idx_blocks_inode ON blocks(inode_id);
CREATE INDEX idx_layer_entries_layer_path ON layer_entries(layer_id, path);

-- 复合索引（优化常用查询路径）
CREATE INDEX idx_inodes_parent_name ON inodes(parent_id, name) 
WHERE is_deleted = FALSE;

-- 部分索引（过滤常见条件）
CREATE INDEX idx_inodes_active ON inodes(inode_id) 
WHERE is_deleted = FALSE;

-- 2. 查询重写
-- 避免 SELECT *，只查询需要的字段
SELECT inode_id, name, type, size FROM inodes WHERE parent_id = ?;

-- 3. 批量操作
-- 使用 INSERT ... VALUES (...), (...), (...)
-- 或 COPY 命令

-- 4. 准备语句
-- 重用查询计划，减少解析开销
PREPARE get_inode AS SELECT * FROM inodes WHERE inode_id = $1;
EXECUTE get_inode(12345);

-- 5. 使用 CTE 优化递归查询
WITH RECURSIVE layer_chain AS (
    SELECT layer_id, parent_layer_id, 0 as depth
    FROM layers WHERE layer_id = $1
    UNION ALL
    SELECT l.layer_id, l.parent_layer_id, lc.depth + 1
    FROM layers l
    JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
    WHERE lc.depth < 100  -- 限制递归深度
)
SELECT * FROM layer_chain;
```

### 分区表优化

```sql
-- 按时间分区（审计日志）
CREATE TABLE audit_logs (
    ...
    log_date DATE NOT NULL DEFAULT CURRENT_DATE
) PARTITION BY RANGE (log_date);

-- 自动创建分区
CREATE OR REPLACE FUNCTION create_partition(table_name text, start_date date)
RETURNS void AS $$
DECLARE
    partition_name text;
    end_date date;
BEGIN
    partition_name := table_name || '_' || to_char(start_date, 'YYYY_MM');
    end_date := start_date + interval '1 month';
    
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF %I FOR VALUES FROM (%L) TO (%L)',
        partition_name, table_name, start_date, end_date
    );
END;
$$ LANGUAGE plpgsql;

-- 自动清理旧分区
CREATE OR REPLACE FUNCTION drop_old_partitions(table_name text, retention_days int)
RETURNS void AS $$
DECLARE
    partition record;
BEGIN
    FOR partition IN
        SELECT tablename FROM pg_tables
        WHERE schemaname = 'public'
        AND tablename LIKE table_name || '_%'
        AND tablename < table_name || '_' || to_char(now() - retention_days * interval '1 day', 'YYYY_MM')
    LOOP
        EXECUTE format('DROP TABLE IF EXISTS %I', partition.tablename);
    END LOOP;
END;
$$ LANGUAGE plpgsql;
```

### PostgreSQL 配置优化

```ini
# 内存配置
shared_buffers = 4GB              # 共享缓冲区（服务器内存的 25%）
effective_cache_size = 12GB       # 操作系统缓存（服务器内存的 75%）
work_mem = 256MB                  # 每个操作的工作内存
maintenance_work_mem = 1GB        # 维护操作内存

# WAL 配置
wal_buffers = 16MB
checkpoint_timeout = 10min
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB

# 并发配置
max_connections = 200
max_parallel_workers_per_gather = 4
max_parallel_workers = 8

# 查询优化
random_page_cost = 1.1            # SSD 降低随机读成本
effective_io_concurrency = 200    # SSD 可以处理更多并发 I/O
default_statistics_target = 100   # 增加统计样本

# 自动清理
autovacuum = on
autovacuum_max_workers = 3
autovacuum_naptime = 10s
```

## I/O 优化

### 批量操作

```rust
// 批量读取数据块
async fn read_blocks_batch(block_ids: Vec<BlockId>) -> Result<Vec<BlockData>> {
    // 1. 检查缓存
    let (cached, uncached): (Vec<_>, Vec<_>) = block_ids
        .into_iter()
        .partition(|id| cache.contains_key(id));
    
    // 2. 从缓存读取
    let mut results = cached
        .into_iter()
        .map(|id| cache.get(&id).unwrap())
        .collect::<Vec<_>>();
    
    // 3. 批量查询未缓存的块
    if !uncached.is_empty() {
        let query = "SELECT * FROM blocks WHERE block_id = ANY($1)";
        let db_results = sqlx::query_as(query)
            .bind(&uncached)
            .fetch_all(&pool)
            .await?;
        
        // 4. 更新缓存
        for block in &db_results {
            cache.insert(block.block_id, block.clone());
        }
        
        results.extend(db_results);
    }
    
    Ok(results)
}

// 批量写入数据块
async fn write_blocks_batch(blocks: Vec<BlockData>) -> Result<()> {
    // 使用 COPY 协议批量插入
    let mut copy = conn.copy_in("COPY blocks FROM STDIN").await?;
    
    for block in blocks {
        let row = format!("{}\t{}\t{}\t{}\\n",
            block.block_id,
            block.inode_id,
            block.block_index,
            hex::encode(&block.data)
        );
        copy.write_all(row.as_bytes()).await?;
    }
    
    copy.finish().await?;
    Ok(())
}
```

### 零拷贝优化

```rust
// 使用 io_uring（Linux）
#[cfg(target_os = "linux")]
async fn read_file_zero_copy(path: &Path) -> Result<Vec<u8>> {
    use io_uring::{opcode, types};
    
    let ring = IoUring::new(32)?;
    let fd = std::fs::File::open(path)?;
    
    // 直接从文件描述符读取，避免用户态拷贝
    let mut buf = vec![0u8; 4096];
    let read_e = opcode::Read::new(types::Fd(fd.as_raw_fd()), buf.as_mut_ptr(), buf.len() as _)
        .build()
        .user_data(0x42);
    
    unsafe {
        ring.submission().push(&read_e)?;
    }
    ring.submit_and_wait(1)?;
    
    Ok(buf)
}

// 使用 mmap
fn read_block_mmap(block_id: BlockId) -> Result<&[u8]> {
    // 对于大块数据，使用 mmap 避免拷贝
    let file = open_block_file(block_id)?;
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(&mmap[..])
}
```

### 异步 I/O

```rust
// 使用 tokio 异步运行时
#[tokio::main]
async fn main() {
    // 所有 I/O 操作都是异步的
    let mut tasks = Vec::new();
    
    for path in paths {
        let task = tokio::spawn(async move {
            read_file(path).await
        });
        tasks.push(task);
    }
    
    // 并发执行所有任务
    let results = futures::future::join_all(tasks).await;
}

// I/O 线程池
let io_pool = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(16)  // 16 个 I/O 线程
    .thread_name("tarbox-io")
    .build()?;
```

## 并发控制

### 锁策略

```rust
// 细粒度锁
struct InodeLockManager {
    locks: DashMap<InodeId, RwLock<()>>,
}

impl InodeLockManager {
    // 文件级读锁
    async fn read_lock(&self, inode_id: InodeId) -> ReadGuard {
        self.locks
            .entry(inode_id)
            .or_insert_with(|| RwLock::new(()))
            .read()
            .await
    }
    
    // 文件级写锁
    async fn write_lock(&self, inode_id: InodeId) -> WriteGuard {
        self.locks
            .entry(inode_id)
            .or_insert_with(|| RwLock::new(()))
            .write()
            .await
    }
}

// 使用示例
async fn write_file(inode_id: InodeId, data: &[u8]) -> Result<()> {
    let _lock = lock_manager.write_lock(inode_id).await;
    // 持有写锁期间执行写入
    do_write(inode_id, data).await?;
    Ok(())
    // 锁自动释放
}
```

### 乐观并发控制

```rust
// 使用版本号实现乐观锁
struct Inode {
    inode_id: InodeId,
    data: InodeData,
    version: i64,  // 版本号
}

async fn update_inode_optimistic(inode: &mut Inode) -> Result<()> {
    loop {
        // 1. 读取当前版本
        let current_version = inode.version;
        
        // 2. 执行修改
        inode.data.size += 1024;
        
        // 3. 尝试更新（检查版本）
        let query = "UPDATE inodes SET data = $1, version = version + 1 
                     WHERE inode_id = $2 AND version = $3";
        let rows_affected = sqlx::query(query)
            .bind(&inode.data)
            .bind(inode.inode_id)
            .bind(current_version)
            .execute(&pool)
            .await?
            .rows_affected();
        
        if rows_affected > 0 {
            // 成功
            inode.version += 1;
            return Ok(());
        }
        
        // 4. 版本冲突，重试
        // 重新读取最新版本
        *inode = load_inode(inode.inode_id).await?;
        // 继续循环重试
    }
}
```

### 无锁数据结构

```rust
// 使用 crossbeam 的无锁数据结构
use crossbeam::queue::SegQueue;

struct AuditQueue {
    queue: Arc<SegQueue<AuditEvent>>,
}

impl AuditQueue {
    fn push(&self, event: AuditEvent) {
        self.queue.push(event);  // 无锁 push
    }
    
    fn pop(&self) -> Option<AuditEvent> {
        self.queue.pop()  // 无锁 pop
    }
}

// 无锁计数器
use std::sync::atomic::{AtomicU64, Ordering};

struct Statistics {
    read_count: AtomicU64,
    write_count: AtomicU64,
}

impl Statistics {
    fn increment_read(&self) {
        self.read_count.fetch_add(1, Ordering::Relaxed);
    }
}
```

## 内存优化

### 内存池

```rust
// 使用对象池避免频繁分配
use object_pool::Pool;

lazy_static! {
    static ref BUFFER_POOL: Pool<Vec<u8>> = Pool::new(1000, || {
        Vec::with_capacity(4096)
    });
}

fn read_block() -> Result<Vec<u8>> {
    // 从池中获取 buffer
    let mut buffer = BUFFER_POOL.pull();
    
    // 使用 buffer
    read_data_into(&mut buffer)?;
    
    Ok(buffer)
    // buffer 自动归还到池中
}
```

### 零分配路径

```rust
// 使用引用避免克隆
fn resolve_path<'a>(path: &'a Path, cache: &'a Cache) -> Result<&'a Inode> {
    // 返回引用而不是克隆
    cache.get(path).ok_or(Error::NotFound)
}

// 使用 Cow 延迟克隆
use std::borrow::Cow;

fn normalize_path(path: &str) -> Cow<str> {
    if path.starts_with('/') {
        // 无需修改，返回借用
        Cow::Borrowed(path)
    } else {
        // 需要修改，返回拥有的数据
        Cow::Owned(format!("/{}", path))
    }
}
```

### 内存限制

```rust
// 监控内存使用
struct MemoryMonitor {
    current_usage: AtomicUsize,
    max_usage: usize,
}

impl MemoryMonitor {
    fn try_allocate(&self, size: usize) -> Result<()> {
        let current = self.current_usage.load(Ordering::Relaxed);
        if current + size > self.max_usage {
            return Err(Error::OutOfMemory);
        }
        
        self.current_usage.fetch_add(size, Ordering::Relaxed);
        Ok(())
    }
    
    fn deallocate(&self, size: usize) {
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
    }
}
```

## 性能测试

### 基准测试

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_read_file(c: &mut Criterion) {
    let fs = setup_filesystem();
    
    c.bench_function("read_4kb", |b| {
        b.iter(|| {
            fs.read(black_box("/data/file.txt"), 0, 4096)
        });
    });
    
    c.bench_function("read_1mb", |b| {
        b.iter(|| {
            fs.read(black_box("/data/large.bin"), 0, 1024 * 1024)
        });
    });
}

criterion_group!(benches, bench_read_file);
criterion_main!(benches);
```

### 压力测试

```bash
# 使用 fio 进行文件系统压力测试
fio --name=seq_read \
    --directory=/mnt/tarbox \
    --rw=read \
    --bs=1m \
    --size=10g \
    --numjobs=4 \
    --runtime=60

# 使用 sysbench 测试数据库
sysbench --test=oltp \
    --db-driver=pgsql \
    --pgsql-host=localhost \
    --pgsql-db=tarbox \
    --num-threads=16 \
    run
```

### 性能监控

```rust
// 使用 Prometheus 指标
use prometheus::{Histogram, Counter};

lazy_static! {
    static ref OP_DURATION: Histogram = register_histogram!(
        "tarbox_operation_duration_seconds",
        "Operation duration"
    ).unwrap();
    
    static ref OP_COUNTER: Counter = register_counter!(
        "tarbox_operations_total",
        "Total operations"
    ).unwrap();
}

async fn monitored_read(path: &Path) -> Result<Vec<u8>> {
    let timer = OP_DURATION.start_timer();
    let result = read_file(path).await;
    timer.observe_duration();
    OP_COUNTER.inc();
    result
}
```

## 性能调优建议

### 硬件建议

```
CPU：
- 推荐：16+ 核心
- 高频率优于多核心（单线程性能重要）

内存：
- 推荐：32GB+
- 用于缓存高频数据和元数据

存储：
- PostgreSQL：NVMe SSD
- 缓存：SSD
- IOPS：10k+ （SSD）
- 吞吐：1GB/s+

网络：
- 10Gbps+
- 低延迟（< 1ms RTT）
```

### 配置建议

```toml
# 生产环境配置
[cache]
metadata_cache_size = "4GB"
block_cache_size = "8GB"
path_cache_size = "2GB"

[database]
pool_size = 50
max_lifetime = "30m"

[performance]
readahead_size = "2MB"
write_buffer_size = "4MB"
io_threads = 16
worker_threads = 8

[fuse]
max_readahead = "1MB"
max_write = "1MB"
async_read = true
```

## 性能问题排查

### 诊断工具

```bash
# 查看缓存命中率
tarbox stats cache

# 查看高频访问文件
tarbox stats frequent-files --top 100

# 查看慢查询
tarbox stats slow-queries --threshold 100ms

# 数据库分析
EXPLAIN ANALYZE SELECT ...;
```

### 常见性能问题

```
问题 1：缓存命中率低
原因：工作集大于缓存大小
解决：增大缓存或优化访问模式

问题 2：写入慢
原因：同步写入、锁竞争
解决：启用写缓存、优化锁粒度

问题 3：元数据操作慢
原因：数据库查询慢、索引缺失
解决：添加索引、优化查询

问题 4：高并发下性能下降
原因：锁竞争、连接池耗尽
解决：增加连接池、使用乐观锁
```

## 文本文件优化

### 文本文件缓存策略

```
文本文件有独特的访问模式，需要专门的缓存策略：

1. LineMap 缓存：
   - 缓存热门文件的 text_line_map
   - 避免每次读取都查询数据库
   - 容量：10000 个文件的 line_map
   - 淘汰：LRU

2. TextBlock 缓存：
   - 缓存常用的 text_blocks
   - 跨文件共享相同内容
   - 容量：100MB（约 25000 个 4KB 块）
   - 淘汰：LRU

3. 重组结果缓存：
   - 缓存完整重组后的文件内容
   - 避免重复重组开销
   - 容量：50MB（约 50 个 1MB 文件）
   - TTL：10 分钟或写入时失效
   - 淘汰：LRU

配置示例：
[cache.text_files]
line_map_cache_size = 10000        # 文件数
text_block_cache_size_mb = 100      # MB
reconstructed_cache_size_mb = 50    # MB
reconstructed_cache_ttl = "10m"     # 分钟
```

### 文本文件读取优化

```
优化策略：

1. 批量查询：
   - 一次查询获取文件的所有 line_map
   - 批量读取所有需要的 text_blocks
   - 减少数据库往返次数

2. 预读优化：
   - 检测顺序读取模式
   - 预读后续的 line_map 和 blocks
   - 适合日志和代码阅读场景

3. 部分读取：
   - 支持只读取文件的一部分行
   - 只查询和加载需要的 blocks
   - 适合大文件的部分查看

4. 并行重组：
   - TextBlock 的读取可以并行
   - 利用多核提升重组速度
   - 适合超大文件

读取流程：
1. 检查重组结果缓存
2. 查询或缓存中获取 line_map
3. 提取所需的 block_ids（去重）
4. 批量读取 TextBlocks（优先从缓存）
5. 从数据库读取缺失的 blocks
6. 重组文件内容
7. 缓存重组结果
8. 返回请求的数据切片
```

### 文本文件写入优化

```
优化策略：

1. 差异计算优化：
   - 使用高效的 diff 算法（Myers）
   - 对大文件采样计算
   - 异步计算（不阻塞写入）

2. 批量操作：
   - 批量创建 TextBlock
   - 批量插入 line_map
   - 使用事务保证原子性

3. 去重优化：
   - 先计算所有 block 的哈希
   - 批量查询哪些已存在
   - 只插入新的 blocks

4. 缓存更新：
   - 写入时更新 LineMap 缓存
   - 失效重组结果缓存
   - 智能更新 TextBlock 缓存

写入流程：
1. 解析新内容为行
2. 如果有父层，计算与父层的 diff
3. 将新内容分块并计算哈希
4. 批量检查哪些 block 已存在
5. 插入新的 blocks
6. 创建完整的 line_map
7. 批量插入 line_map
8. 更新缓存
9. 更新 metadata
```

### 性能指标

```
文本文件操作性能目标：

读取性能：
- 小文件（< 10KB）：< 5ms（包含重组）
- 中文件（10-100KB）：< 20ms
- 大文件（100KB-1MB）：< 100ms
- 缓存命中：< 1ms

写入性能：
- 小修改（< 10 行）：< 30ms
- 中等修改（10-100 行）：< 100ms
- 大修改（> 100 行）：< 500ms
- 新文件创建：< 50ms

缓存命中率：
- LineMap 缓存：> 85%
- TextBlock 缓存：> 80%
- 重组结果缓存：> 70%

存储效率：
- 文本去重率：> 30%（跨文件）
- 层间增量率：> 80%（只存储 20% 变化）
```

### 性能测试场景

```
1. 代码编辑场景：
   - 频繁修改少量行
   - 预期：写入 < 30ms，读取 < 5ms

2. 日志追加场景：
   - 连续追加新行
   - 预期：追加 < 10ms，读取最新 < 5ms

3. 配置文件场景：
   - 偶尔修改几行
   - 预期：写入 < 50ms，读取 < 2ms

4. 大文件浏览场景：
   - 只读取部分内容
   - 预期：部分读取 < 20ms

5. 多层历史查询：
   - 在不同层间切换
   - 预期：切换 + 读取 < 50ms
```

