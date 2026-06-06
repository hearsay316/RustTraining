# 总结和参考卡

## 快速参考卡

### 异步心理模型

```text
┌─────────────────────────────────────────────────────┐
│  async fn → State Machine (enum) → impl Future     │
│  .await   → poll() the inner future                 │
│  executor → loop { poll(); sleep_until_woken(); }   │
│  waker    → "hey executor, poll me again"           │
│  Pin      → "promise I won't move in memory"        │
└─────────────────────────────────────────────────────┘
```

### 常见模式备忘单

| 目标 | 使用 |
|------|-----|
| 同时运行两个 future | `tokio::join!(a, b)` |
| 比赛两个Future | `tokio::select! { ... }` |
| 生成后台任务 | `tokio::spawn(async { ... })` |
| 以异步方式运行阻塞代码 | `tokio::任务::spawn_blocking(\|\| { ... })` |
| 限制并发数 | `Semaphore::new(N)` |
| 收集大量任务结果 | `JoinSet` |
| 跨任务共享状态 | `Arc<Mutex<T>>` 或频道 |
| 优雅关机 | `watch::channel` + `select!` |
| 一次处理 N 个流 | `.buffer_unordered(N)` |
| 超时Future | `tokio::time::timeout(dur, fut)` |
| 后退重试 | 自定义组合器（参见第 13 章） |

### 固定快速参考

| 情况 | 使用 |
|-----------|-----|
| Pin 堆上的Future | `Box::pin(fut)` |
| Pin 栈上的Future | `tokio::pin!(fut)` |
| Pin `Unpin` 类型 | `Pin::new(&mut val)` — 安全、免费 |
| 返回固定的 trait 对象 | `-> Pin<Box<dyn Future<Output = T> + Send>>` |

### 渠道选择指南

| 渠道 | 制片人 | 消费者 | 价值观 | 使用时间 |
|---------|-----------|-----------|--------|----------|
| `mpsc` | 氮 | 1 | Stream | 工作队列、事件总线 |
| `oneshot` | 1 | 1 | 单身的 | 请求/响应、完成通知 |
| `broadcast` | 氮 | 氮 | 全部接收全部 | 扇出通知、关闭信号 |
| `watch` | 1 | 氮 | 仅最新 | 配置更新、健康状态 |

### Mutex 选型指南

| Mutex | 使用时间 |
|-------|----------|
| `std::sync::Mutex` | 锁被短暂持有，永远不会跨越`.await` |
| `tokio::sync::Mutex` | 锁必须跨过`.await` |
| `parking_lot::Mutex` | 高竞争，无`.await`，需要性能 |
| `tokio::sync::RwLock` | 读者多，作者少，锁交叉`.await` |

### 决策快速参考

```text
Need concurrency?
├── I/O-bound → async/await
├── CPU-bound → rayon / std::thread
└── Mixed → spawn_blocking for CPU parts

Choosing runtime?
├── Server app → tokio
├── Library → runtime-agnostic (futures crate)
├── Embedded → embassy
└── Minimal → smol

Need concurrent futures?
├── Can be 'static + Send → tokio::spawn
├── Can be 'static + !Send → LocalSet
├── Can't be 'static → FuturesUnordered
└── Need to track/abort → JoinSet
```

### 常见错误消息和修复

| 错误 | 原因 | 使固定 |
|-------|-------|-----|
| `future is not Send` | 在 `.await` 上按住 `!Send` 字 | 调整值的范围，使其在 `.await` 之前被删除，或使用 `current_thread` Runtime |
| `borrowed value does not live long enough` 生命周期不足 | `tokio::spawn`需要`'static` | 使用 `Arc`、`clone()` 或 `FuturesUnordered` |
| `the trait Future is not implemented for ()` | 缺少 `.await` | 将 `.await` 添加到异步调用中 |
| `poll` 中 `cannot borrow as mutable` | 自引用借用 | 正确使用 `Pin<&mut Self>`（参见第 4 章） |
| 程序静静地挂起 | 忘记打电话`waker.wake()` | 确保每个`Pending`路径注册并触发Waker |

### 进一步阅读

| 资源 | 为什么 |
|----------|-----|
| [Tokio 教程](https://tokio.rs/tokio/tutorial) | 官方实践指南——非常适合第一个项目 |
| [异步书（官方）](https://rust-lang.github.io/async-book/) | 涵盖语言级别的`Future`、`Pin`、`Stream` |
| [Jon Gjengset — Rust 的外壳：async/await](https://www.youtube.com/watch?v=ThjvMReOXYM) | 通过实时编码进行 2 小时深入了解内部结构 |
| [Alice Ryhl — Tokio Actor 模式](https://ryhl.io/blog/actors-with-tokio/) | 有状态服务的生产架构模式 |
| [没有船 - Pin、Unpin，以及为什么 Rust 需要它们](https://without.boats/blog/pin/) | 语言设计者的最初动机 |
| [Tokio 迷你Redis](https://github.com/tokio-rs/mini-redis) | 完成异步 Rust项目——研究质量的生产代码 |
| [Tower 文档](https://docs.rs/tower) | axum、tonic、hyper 使用的中间件/服务架构 |

***

*异步结束Rust培训指南*

