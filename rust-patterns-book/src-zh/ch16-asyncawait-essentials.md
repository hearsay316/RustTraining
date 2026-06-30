# 16. Async/Await 基础 🔴

> **你将学到：**
> - Rust 的 `Future` trait 与 Go 的 goroutine 和 Python 的 asyncio 有何不同
> - Tokio 快速入门：派生任务、`join!` 和运行时配置
> - 常见 async 陷阱及其修复方法
> - 何时使用 `spawn_blocking` 卸载阻塞式工作

## Future、运行时与 `async fn`

Rust 的 async 模型与 Go 的 goroutine 或 Python 的 `asyncio` *根本不同*。
理解三个概念就足以入门：

1. **`Future` 是一个惰性的状态机** — 调用 `async fn` 不会执行任何东西；
   它返回一个必须被轮询（poll）的 `Future`。
2. **你需要一个运行时**来轮询 future — `tokio`、`async-std` 或 `smol`。
   标准库定义了 `Future` 但不提供运行时。
3. **`async fn` 是语法糖** — 编译器将其转换为实现 `Future` 的状态机。

```rust
// Future 只是一个 trait：
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

// async fn 脱糖为：
// fn fetch_data(url: &str) -> impl Future<Output = Result<Vec<u8>, Error>>
async fn fetch_data(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = reqwest::get(url).await?;  // .await 在就绪前让出
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
```

### Tokio 快速入门

```toml
# Cargo.toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

```rust,ignore
use tokio::time::{sleep, Duration};
use tokio::task;

#[tokio::main]
async fn main() {
    // 派生并发任务（类似轻量级线程）：
    let handle_a = task::spawn(async {
        sleep(Duration::from_millis(100)).await;
        "task A done"
    });

    let handle_b = task::spawn(async {
        sleep(Duration::from_millis(50)).await;
        "task B done"
    });

    // 同时 .await 它们 — 它们并发运行，而非顺序运行：
    let (a, b) = tokio::join!(handle_a, handle_b);
    println!("{}, {}", a.unwrap(), b.unwrap());
}
```

### Async 常见陷阱

| 陷阱 | 原因 | 修复 |
|---------|---------------|-----|
| 在 async 中阻塞 | `std::thread::sleep` 或 CPU 密集工作阻塞执行器 | 使用 `tokio::task::spawn_blocking` 或 `rayon` |
| `Send` 约束错误 | 跨 `.await` 持有的 Future 包含 `!Send` 类型（如 `Rc`、`MutexGuard`） | 重构以在 `.await` 前丢弃非 Send 值 |
| Future 未被轮询 | 调用 `async fn` 却不 `.await` 或 spawn — 什么都不会发生 | 总是 `.await` 或 `tokio::spawn` 返回的 future |
| 跨 `.await` 持有 `MutexGuard` | `std::sync::MutexGuard` 是 `!Send`；async 任务可能在不同线程恢复 | 使用 `tokio::sync::Mutex` 或在 `.await` 前丢弃守卫 |
| 意外的顺序执行 | `let a = foo().await; let b = bar().await;` 是顺序运行 | 使用 `tokio::join!` 或 `tokio::spawn` 实现并发 |

```rust
// ❌ 阻塞 async 执行器：
async fn bad() {
    std::thread::sleep(std::time::Duration::from_secs(5)); // 阻塞整个线程！
}

// ✅ 卸载阻塞式工作：
async fn good() {
    tokio::task::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_secs(5)); // 在阻塞池上运行
    }).await.unwrap();
}
```

> **全面的 async 覆盖**：关于 `Stream`、`select!`、取消安全性、
> 结构化并发和 `tower` 中间件，请参阅我们专门的
> **Async Rust 训练**指南。本节只覆盖读写基本 async 代码所需的内容。

### 派生与结构化并发

Tokio 的 `spawn` 创建一个新的异步任务 — 类似 `thread::spawn` 但轻量得多：

```rust,ignore
use tokio::task;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    // 派生三个并发任务
    let h1 = task::spawn(async {
        sleep(Duration::from_millis(200)).await;
        "fetched user profile"
    });

    let h2 = task::spawn(async {
        sleep(Duration::from_millis(100)).await;
        "fetched order history"
    });

    let h3 = task::spawn(async {
        sleep(Duration::from_millis(150)).await;
        "fetched recommendations"
    });

    // 并发等待三者（而非顺序！）
    let (r1, r2, r3) = tokio::join!(h1, h2, h3);
    println!("{}", r1.unwrap());
    println!("{}", r2.unwrap());
    println!("{}", r3.unwrap());
}
```

**`join!` vs `try_join!` vs `select!`**：

| 宏 | 行为 | 何时使用 |
|-------|----------|----------|
| `join!` | 等待所有 future | 所有任务都必须完成 |
| `try_join!` | 等待所有，遇到第一个 `Err` 则短路 | 任务返回 `Result` |
| `select!` | 第一个 future 完成时返回 | 超时、取消 |

```rust,ignore
use tokio::time::{timeout, Duration};

async fn fetch_with_timeout() -> Result<String, Box<dyn std::error::Error>> {
    let result = timeout(Duration::from_secs(5), async {
        // 模拟慢速网络调用
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<_, Box<dyn std::error::Error>>("data".to_string())
    }).await??; // 第一个 ? 解包 Elapsed，第二个 ? 解包内部 Result

    Ok(result)
}
```

### `Send` 约束与为何 Future 必须是 `Send`

当你 `tokio::spawn` 一个 future 时，它可能在不同的 OS 线程上恢复。
这意味着 future 必须是 `Send`。常见陷阱：

```rust,ignore
use std::rc::Rc;

async fn not_send() {
    let rc = Rc::new(42); // Rc 是 !Send
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    println!("{}", rc); // rc 跨 .await 持有 — future 是 !Send
}

// 修复 1：在 .await 前丢弃
async fn fixed_drop() {
    let data = {
        let rc = Rc::new(42);
        *rc // 将值拷贝出来
    }; // rc 在此丢弃
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    println!("{}", data); // 只是一个 i32，是 Send
}

// 修复 2：用 Arc 代替 Rc
async fn fixed_arc() {
    let arc = std::sync::Arc::new(42); // Arc 是 Send
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    println!("{}", arc); // ✅ Future 是 Send
}
```

> **全面的 async 覆盖**：关于 `Stream`、`select!`、取消安全性、
> 结构化并发和 `tower` 中间件，请参阅我们专门的
> **Async Rust 训练**指南。本节只覆盖读写基本 async 代码所需的内容。

> **另见：** [第 5 章 — 通道](ch05-channels-and-message-passing.md) 了解同步通道。[第 6 章 — 并发](ch06-concurrency-vs-parallelism-vs-threads.md) 了解 OS 线程与 async 任务。

> **关键要点 — Async**
> - `async fn` 返回一个惰性的 `Future` — 在你 `.await` 或 spawn 它之前什么都不运行
> - 在 async 上下文中处理 CPU 密集或阻塞工作时使用 `tokio::task::spawn_blocking`
> - 不要跨 `.await` 持有 `std::sync::MutexGuard` — 改用 `tokio::sync::Mutex`
> - spawn 的 Future 必须是 `Send` — 在 `.await` 点之前丢弃 `!Send` 类型

---

### 练习：带超时的并发抓取器 ★★（约 25 分钟）

编写一个 async 函数 `fetch_all`，它派生三个 `tokio::spawn` 任务，每个任务
用 `tokio::time::sleep` 模拟网络调用。用 `tokio::try_join!` 连接三者，
并包裹在 `tokio::time::timeout(Duration::from_secs(5), ...)` 中。
返回 `Result<Vec<String>, ...>`，如果任何任务失败或截止时间过期则返回错误。

<details>
<summary>🔑 解答</summary>

```rust,ignore
use tokio::time::{sleep, timeout, Duration};

async fn fake_fetch(name: &'static str, delay_ms: u64) -> Result<String, String> {
    sleep(Duration::from_millis(delay_ms)).await;
    Ok(format!("{name}: OK"))
}

async fn fetch_all() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let deadline = Duration::from_secs(5);

    let (a, b, c) = timeout(deadline, async {
        let h1 = tokio::spawn(fake_fetch("svc-a", 100));
        let h2 = tokio::spawn(fake_fetch("svc-b", 200));
        let h3 = tokio::spawn(fake_fetch("svc-c", 150));
        tokio::try_join!(h1, h2, h3)
    })
    .await??;

    Ok(vec![a?, b?, c?])
}

#[tokio::main]
async fn main() {
    let results = fetch_all().await.unwrap();
    for r in &results {
        println!("{r}");
    }
}
```

</details>

***
