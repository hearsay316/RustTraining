# 总结与参考卡片

## 快速参考卡片

### 异步（async）心智模型

```text
// ============================================================================
// 异步 Rust 的核心抽象链：从语法糖到底层机制
// ============================================================================
// async fn  → 编译器生成状态机（enum），每个 .await 点 = 一个状态变体
// .await    → 展开为 poll() 调用，检查内层 Future 是否就绪
// 执行器（executor）    → 核心循环：poll 顶层 Future → Pending 则休眠 → Waker 唤醒后重新 poll
// Waker     → 就绪通知机制："嘿执行器，我准备好被 poll 了"
// Pin       → 内存固定保证："我承诺不会被移动到其他地址"
┌─────────────────────────────────────────────────────┐
│  async fn → State Machine (enum) → impl Future     │
│  .await   → poll() the inner future                 │
│  executor → loop { poll(); sleep_until_woken(); }   │
│  waker    → "hey executor, poll me again"           │
│  Pin      → "promise I won't move in memory"        │
└─────────────────────────────────────────────────────┘
```

### 常见模式速查表

| 目标 | 使用 |
|------|-----|
| 同时运行两个 Future | `tokio::join!(a, b)` |
| 竞速两个 Future | `tokio::select! { ... }` |
| 生成后台任务 | `tokio::spawn(async { ... })` |
| 异步执行阻塞代码 | `tokio::task::spawn_blocking(\|\| { ... })` |
| 限制并发数 | `Semaphore::new(N)` |
| 收集大量任务结果 | `JoinSet` |
| 跨任务共享状态 | `Arc<Mutex<T>>` 或 channel |
| 优雅关闭（graceful shutdown） | `watch::channel` + `select!` |
| 一次处理 N 个 Stream 元素 | `.buffer_unordered(N)` |
| 为 Future 加超时 | `tokio::time::timeout(dur, fut)` |
| 带退避的重试 | 自定义组合器（参见第 13 章） |

### Pin 固定快速参考

| 场景 | 使用 |
|-----------|-----|
| 堆上 Pin 一个 Future | `Box::pin(fut)` |
| 栈上 Pin 一个 Future | `tokio::pin!(fut)` |
| Pin 一个 `Unpin` 类型 | `Pin::new(&mut val)` — 安全、零开销 |
| 返回固定 trait 对象 | `-> Pin<Box<dyn Future<Output = T> + Send>>` |

### Channel 选型指南

| Channel | 生产者 | 消费者 | 传递值 | 适用场景 |
|---------|-----------|-----------|--------|----------|
| `mpsc` | N | 1 | Stream 顺序 | 工作队列、事件总线 |
| `oneshot` | 1 | 1 | 单个值 | 请求/响应、完成通知 |
| `broadcast` | N | N | 每个接收者收到所有值 | 扇出通知、关闭信号 |
| `watch` | 1 | N | 仅保留最新值 | 配置更新、健康状态 |

### Mutex 选型指南

| Mutex | 适用场景 |
|-------|----------|
| `std::sync::Mutex` | 锁持有时间极短，绝不会跨越 `.await` |
| `tokio::sync::Mutex` | 锁必须跨越 `.await` 点 |
| `parking_lot::Mutex` | 高竞争、无 `.await`、追求极致性能 |
| `tokio::sync::RwLock` | 读多写少，锁跨越 `.await` |

### 决策快速参考

```text
// ============================================================================
// 异步 Rust 技术选型决策树
// ============================================================================
// 并发的类型：
//   I/O 密集型 → async/await（千万级并发连接，不阻塞线程）
//   CPU 密集型 → rayon / std::thread（充分利用多核计算）
//   混合型     → spawn_blocking 处理 CPU 部分，其余走 async
//
// 运行时（runtime）选择：
//   tokio    → 服务端应用首选，生态最完善
//   futures  → 库代码，运行时无关（不绑定特定运行时）
//   embassy  → 嵌入式 / no_std 环境
//   smol     → 最小化依赖，适合简单场景
//
// 并发 Future 管理：
//   tokio::spawn     → 'static + Send（可跨线程调度，最灵活）
//   LocalSet         → 'static + !Send（单线程，适合 !Send 类型）
//   FuturesUnordered → 非 'static（借用局部变量，无法 spawn）
//   JoinSet          → 需要追踪/取消任务（比 Vec<JoinHandle> 更高效）

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

### 常见错误消息与修复方法

| 错误 | 原因 | 修复方法 |
|-------|-------|-----|
| `future is not Send` | 在 `.await` 点持有 `!Send` 值 | 调整作用域，让 `!Send` 值在 `.await` 之前 drop，或使用 `current_thread` 运行时 |
| `borrowed value does not live long enough` | `tokio::spawn` 要求 `'static` 生命周期 | 使用 `Arc`、`clone()` 或将非 'static Future 放入 `FuturesUnordered` |
| `the trait Future is not implemented for ()` | 遗漏了 `.await` | 对异步函数调用添加 `.await` |
| `poll` 中 `cannot borrow as mutable` | 自引用结构体的借用冲突 | 正确使用 `Pin<&mut Self>`（参见第 4 章） |
| 程序静默挂起 | 忘记调用 `waker.wake()` | 确保每个 `Pending` 路径都注册了 Waker 并在就绪时触发 |

### 延伸阅读

| 资源 | 推荐理由 |
|----------|-----|
| [Tokio 教程](https://tokio.rs/tokio/tutorial) | 官方实践指南——适合第一个项目入手 |
| [异步书（官方）](https://rust-lang.github.io/async-book/) | 涵盖语言级别的 `Future`、`Pin`、`Stream` |
| [Jon Gjengset — Crust of Rust: async/await](https://www.youtube.com/watch?v=ThjvMReOXYM) | 通过实时编码深入内部原理，2 小时深度讲解 |
| [Alice Ryhl — Tokio Actor 模式](https://ryhl.io/blog/actors-with-tokio/) | 有状态服务的生产级架构模式 |
| [Without Boats — Pin、Unpin，以及为什么 Rust 需要它们](https://without.boats/blog/pin/) | 语言设计者的原始动机与设计考量 |
| [Tokio mini-redis](https://github.com/tokio-rs/mini-redis) | 完整的异步 Rust 项目——研究级生产代码 |
| [Tower 文档](https://docs.rs/tower) | axum、tonic、hyper 等框架使用的中间件/服务架构 |

***

*异步 Rust 培训指南到此结束*

