# 13. 生产模式🔴

> **您将学到什么：**
> - 正常关闭`watch`通道和`select!`
> - 背压：有界通道防止 OOM
> - 结构化并发：`JoinSet` 和 `TaskTracker`
> - 超时、重试和指数退避
> - 错误处理：`thiserror` vs `anyhow`，双`?`模式
> - Tower：axum、tonic、hyper 使用的中间件模式

## 优雅关机

生产服务器必须彻底关闭——完成正在进行的请求、刷新缓冲区、关闭连接：

```rust
use tokio::signal;
use tokio::sync::watch;

async fn main_server() {
    // 创建关闭信号通道
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // 生成服务器
    let server_handle = tokio::spawn(run_server(shutdown_rx.clone()));

    // 等待 Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Shutdown signal received, finishing in-flight requests...");

    // 通知所有任务关闭
    // 注意：使用 .unwrap() 是为了简洁。生产代码应该处理
    // 所有接收器都已被丢弃的情况。
    shutdown_tx.send(true).unwrap();

    // 等待服务器完成（有超时）
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        server_handle,
    ).await {
        Ok(Ok(())) => println!("Server shut down gracefully"),
        Ok(Err(e)) => eprintln!("Server error: {e}"),
        Err(_) => eprintln!("Server shutdown timed out — forcing exit"),
    }
}

async fn run_server(mut shutdown: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            // 接受新连接
            conn = accept_connection() => {
                let shutdown = shutdown.clone();
                tokio::spawn(handle_connection(conn, shutdown));
            }
            // 关机信号
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    println!("Stopping accepting new connections");
                    break;
                }
            }
        }
    }
    // 飞行中的转机将自行完成
    // 因为他们有自己的 shutdown_rx 克隆
}

async fn handle_connection(conn: Connection, mut shutdown: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            request = conn.next_request() => {
                // 完全处理请求——不要中途放弃请求
                process_request(request).await;
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    // 完成当前请求，然后退出
                    break;
                }
            }
        }
    }
}
```

```mermaid
sequenceDiagram
    participant OS as OS 信号
    participant Main as 主 Task
    participant WCH as watch 通道
    participant W1 as Worker 1
    participant W2 as Worker 2

    OS->>Main: SIGINT (Ctrl+C)
    Main->>WCH: send(true)
    WCH-->>W1: changed()
    WCH-->>W2: changed()

    否te over W1: 完成当前请求
    否te over W2: 完成当前请求

    W1-->>Main: Task 完成
    W2-->>Main: Task 完成
    Main->>Main: 所有 Worker 完成 → 退出
```

### 有界通道的背压

如果生产者比消费者更快，无界通道可能会导致 OOM。在生产中始终使用有界通道：

```rust
use tokio::sync::mpsc;

async fn backpressure_example() {
    // 有界通道：最多缓冲 100 个项目
    let (tx, mut rx) = mpsc::channel::<WorkItem>(100);

    // 生产者：缓冲区满时自然变慢
    let producer = tokio::spawn(async move {
        for i in 0..1_000_000 {
            // send() 是 async；如果缓冲区已满就等待
            // 这创造了自然的backpressure!
            tx.send(WorkItem { id: i }).await.unwrap();
        }
    });

    // 消费者：按自己的节奏处理元素
    let consumer = tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            process(item).await; // 处理速度慢是可以的——生产者等待
        }
    });

    let _ = tokio::join!(producer, consumer);
}

// 与无界相比——危险：
// let (tx, rx) = mpsc::unbounded_channel(); // 没有背压！
// 生产者可以无限期地填充内存
```

### 结构化并发：JoinSet 和 TaskTracker

`JoinSet` 对相关任务进行分组并确保它们全部完成：

```rust
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};

async fn structured_concurrency() {
    let mut set = JoinSet::new();

    // 产生一批任务
    for url in get_urls() {
        set.spawn(async move {
            fetch_and_process(url).await
        });
    }

    // 收集所有结果（不保证顺序）
    let mut results = Vec::new();
    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok(data)) => results.push(data),
            Ok(Err(e)) => eprintln!("Task error: {e}"),
            Err(e) => eprintln!("Task panicked: {e}"),
        }
    }

    // 所有任务都在这里完成——没有悬而未决的后台工作
    println!("Processed {} items", results.len());
}

// TaskTracker (tokio-util 0.7.9+) — 等待所有生成的任务
use tokio_util::task::TaskTracker;

async fn with_tracker() {
    let tracker = TaskTracker::new();

    for i in 0..10 {
        tracker.spawn(async move {
            sleep(Duration::from_millis(100 * i)).await;
            println!("Task {i} done");
        });
    }

    tracker.close(); // 不会再添加更多任务
    tracker.wait().await; // 等待所有跟踪的任务
    println!("All tasks finished");
}
```

### 超时和重试

```rust
use tokio::time::{timeout, sleep, Duration};

// 简单的超时
async fn with_timeout() -> Result<Response, Error> {
    match timeout(Duration::from_secs(5), fetch_data()).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(Error::Fetch(e)),
        Err(_) => Err(Error::Timeout),
    }
}

// 指数退避重试
async fn retry_with_backoff<F, Fut, T, E>(
    max_attempts: u32,
    base_delay_ms: u64,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = Duration::from_millis(base_delay_ms);

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_attempts {
                    eprintln!("Final attempt {attempt} failed: {e}");
                    return Err(e);
                }
                eprintln!("Attempt {attempt} failed: {e}, retrying in {delay:?}");
                sleep(delay).await;
                delay *= 2; // 指数退避
            }
        }
    }
    unreachable!()
}

// 用法：
// let result = retry_with_backoff(3, 100, || async {
//     reqwest::get("https://api.example.com/data").await
// }).await?;
```

> **生产技巧 - 添加抖动**：上面的函数使用纯指数退避，但在
> 生产中许多客户端同时失败将以相同的时间间隔重试（雷霆
> 一群）。添加随机 *抖动* — 例如，`sleep(delay + rand_jitter)`，其中 `rand_jitter` 是
> `0..delay/4` — 因此重试会随着时间的推移而分散。

### 异步代码中的错误处理

异步引入了独特的错误传播挑战 - 生成的任务创建错误边界，超时错误包装内部错误，并且当 future 跨越任务边界时，`?` 会以不同的方式交互。

**`thiserror` 与 `anyhow`** — 选择正确的工具：

```rust
// thiserror：适合为库和公开 API 定义类型化错误
// 每个变体都是明确的——调用者可以匹配特定的错误
use thiserror::Error;

#[derive(Error, Debug)]
enum DiagError {
    #[error("IPMI command failed: {0}")]
    Ipmi(#[from] IpmiError),

    #[error("Sensor {sensor} out of range: {value}°C (max {max}°C)")]
    OverTemp { sensor: String, value: f64, max: f64 },

    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Task panicked: {0}")]
    TaskPanic(#[from] tokio::task::JoinError),
}

// anyhow：适合应用程序和原型快速处理错误
// 包装任何错误 - 无需为每种情况定义类型
use anyhow::{Context, Result};

async fn run_diagnostics() -> Result<()> {
    let config = load_config()
        .await
        .context("Failed to load diagnostic config")?;  // 添加上下文

    let result = run_gpu_test(&config)
        .await
        .context("GPU diagnostic failed")?;              // 链上下文

    Ok(())
}
// anyhow 会打印：“GPU 诊断失败：IPMI 命令失败：超时”
```

| 箱 | 使用时间 | 错误类型 | 匹配 |
|-------|----------|-----------|----------|
| `thiserror` | 库代码、公共 API | `enum MyError { ... }` | `match err { MyError::Timeout => ... }` |
| `anyhow` | 应用程序、CLI 工具、脚本 | `anyhow::Error`（类型擦除） | `err.downcast_ref::<MyError>()` |
| 两者都在一起 | 库公开 `thiserror`，应用程序用 `anyhow` 包装 | 两全其美 | 输入库错误，应用程序不关心 |

**双`?`模式**与`tokio::spawn`：

```rust
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
enum AppError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Task panicked: {0}")]
    TaskPanic(#[from] JoinError),
}

async fn spawn_with_errors() -> Result<String, AppError> {
    let handle = tokio::spawn(async {
        let resp = reqwest::get("https://example.com").await?;
        Ok::<_, reqwest::Error>(resp.text().await?)
    });

    // 双？：首先？打开 JoinError （任务恐慌），第二个？解开内部结果
    let result = handle.await??;
    Ok(result)
}
```

**错误边界问题** — `tokio::spawn` 删除上下文：

```rust
// ❌ 错误上下文在 spawn 边界处丢失：
async fn bad_error_handling() -> Result<()> {
    let handle = tokio::spawn(async {
        some_fallible_work().await  // 返回 Result<T, SomeError>
    });

    // handle.await 返回 Result<Result<T, SomeError>, JoinError>
    // 内部错误没有关于失败任务的上下文
    let result = handle.await??;
    Ok(())
}

// ✅ 在 spawn 边界内补充上下文：
async fn good_error_handling() -> Result<()> {
    let handle = tokio::spawn(async {
        some_fallible_work()
            .await
            .context("worker task failed")  // 穿越边界前Context
    });

    let result = handle.await
        .context("worker task panicked")??;  // 也为 JoinError 添加上下文
    Ok(())
}
```

**超时错误** — 包装与替换：

```rust
use tokio::time::{timeout, Duration};

async fn with_timeout_context() -> Result<String, DiagError> {
    let dur = Duration::from_secs(30);
    match timeout(dur, fetch_sensor_data()).await {
        Ok(Ok(data)) => Ok(data),
        Ok(Err(e)) => Err(e),                      // 保留内部错误
        Err(_) => Err(DiagError::Timeout(dur)),     // 超时→输入错误
    }
}
```

### Tower：中间件模式

[Tower](https://docs.rs/tower) crate 定义了一个可组合的 `Service` trait — Rust 中异步中间件的主干（由 `axum`、`tonic`、`hyper` 使用）：

```rust
// Tower 的核心 trait（简化）：
pub trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    fn call(&mut self, req: Request) -> Self::Future;
}
```

中间件包装了 `Service` 以添加横切行为 - 日志记录、超时、速率限制 - 而不修改内部逻辑：

```rust
use tower::{ServiceBuilder, timeout::TimeoutLayer, limit::RateLimitLayer};
use std::time::Duration;

let service = ServiceBuilder::new()
    .layer(TimeoutLayer::new(Duration::from_secs(10)))       // 最外层：超时
    .layer(RateLimitLayer::new(100, Duration::from_secs(1))) // 然后：速率限制
    .service(my_handler);                                     // 最内层：你的代码
```

**为什么这很重要**：如果您使用过 ASP.NET 中间件或 Express.js 中间件，则 Tower 相当于 Rust。这就是生产 Rust 服务如何在不重复代码的情况下添加横切关注点。

### 练习：使用工作池正常关闭

<details>
<summary>🏋️ 练习（点击展开）</summary>

**挑战**：构建一个任务处理器，具有基于通道的工作队列、N 个工作任务以及按 Ctrl+C 正常关闭。工作人员应在离开前完成飞行中的工作。

<details>
<summary>🔑 参考答案</summary>

```rust
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};

struct WorkItem { id: u64, payload: String }

#[tokio::main]
async fn main() {
    let (work_tx, work_rx) = mpsc::channel::<WorkItem>(100);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let work_rx = std::sync::Arc::new(tokio::sync::Mutex::new(work_rx));

    let mut handles = Vec::new();
    for id in 0..4 {
        let rx = work_rx.clone();
        let mut shutdown = shutdown_rx.clone();
        handles.push(tokio::spawn(async move {
            loop {
                let item = {
                    let mut rx = rx.lock().await;
                    tokio::select! {
                        item = rx.recv() => item,
                        _ = shutdown.changed() => {
                            if *shutdown.borrow() { None } else { continue }
                        }
                    }
                };
                match item {
                    Some(work) => {
                        println!("Worker {id}: processing {}", work.id);
                        sleep(Duration::from_millis(200)).await;
                    }
                    None => break,
                }
            }
        }));
    }

    // 提交作品
    for i in 0..20 {
        let _ = work_tx.send(WorkItem { id: i, payload: format!("task-{i}") }).await;
        sleep(Duration::from_millis(50)).await;
    }

    // 收到 Ctrl+C 时：发出关闭信号，等待 worker
    // 注意：这里用 .unwrap() 是为了简洁；生产代码应处理错误。
    tokio::signal::ctrl_c().await.unwrap();
    shutdown_tx.send(true).unwrap();
    for h in handles { let _ = h.await; }
    println!("Shut down cleanly.");
}
```

</details>
</details>

> **关键要点——生产模式**
> - 使用 `watch` 通道 + `select!` 协调正常关闭
> - 有界通道 (`mpsc::channel(N)`) 提供 **背压** — 当缓冲区已满时发送者会阻塞
> - `JoinSet` 和 `TaskTracker` 提供**结构化并发**：跟踪、中止和等待任务组
> - 始终为网络操作添加超时 — `tokio::time::timeout(dur, fut)`
> - Tower的`Service`trait是生产Rust服务的标准中间件模式

> **另请参阅：** [第 8 章 — Tokio 深入探讨](ch08-tokio-deep-dive.md) 用于通道和同步原语，[第 12 章 — 常见陷阱](ch12-common-pitfalls.md) 用于关闭期间的取消风险

***


