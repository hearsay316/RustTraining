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
// → std::future::Future：Rust 异步的核心抽象。
//   它是一个惰性状态机，poll 返回 Pending（未就绪）或 Ready(value)。
pub trait Future {
    // → 关联类型 Output：Future 完成时产出的值类型。
    type Output;
    // → poll 是驱动 Future 的唯一方法。
    //   self: Pin<&mut Self>：固定可变引用，保证自引用类型不会被移动。
    //   cx: &mut Context：携带 Waker，Future 未就绪时通过它注册唤醒回调。
    //   返回 Poll<T>：Pending（挂起）或 Ready(T)（完成）。
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

// async fn 脱糖为：
// fn fetch_data(url: &str) -> impl Future<Output = Result<Vec<u8>, Error>>
// → async fn 是语法糖：编译器将函数体转换为返回 impl Future 的状态机，
//   输出类型即函数声明的返回类型。调用 async fn 不会执行函数体，
//   而是返回一个需被 poll 的 Future。
async fn fetch_data(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    // → .await：轮询右侧 Future 直到 Ready，其间让出执行权给运行时（协作式调度）。
    //   ? 解包 Result，错误时提前返回。
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
// → tokio::time::{sleep, Duration}：
//   sleep(dur) 返回一个在 dur 后就绪的 Future（异步休眠，不阻塞线程）。
//   Duration 是 std::time::Duration 时间段类型。
use tokio::time::{sleep, Duration};
// → tokio::task：包含 spawn 等任务管理 API。
use tokio::task;

// → #[tokio::main]：属性宏，将 async fn main 展开为同步 main，
//   内部构建 Tokio 运行时并 block_on 异步 main。这是 async 程序的入口约定。
#[tokio::main]
async fn main() {
    // 派生并发任务（类似轻量级线程）：
    // → task::spawn(future)：将 future 提交到运行时并发执行，
    //   返回 JoinHandle<T>（T 是 future 的输出类型）。
    //   任务立即开始调度，无需手动 poll。
    let handle_a = task::spawn(async {
        sleep(Duration::from_millis(100)).await;
        "task A done"
    });

    let handle_b = task::spawn(async {
        sleep(Duration::from_millis(50)).await;
        "task B done"
    });

    // 同时 .await 它们 — 它们并发运行，而非顺序运行：
    // → tokio::join!(f1, f2)：并发轮询多个 future，等待全部完成。
    //   返回元组 (F1::Output, F2::Output)。
    //   与逐个 await 不同，join! 不会阻塞一个 future 阻塞另一个。
    let (a, b) = tokio::join!(handle_a, handle_b);
    // → JoinHandle::await 返回 Result<T, JoinError>，
    //   unwrap() 解包；若任务 panic 则返回 Err。
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
    // → std::thread::sleep：同步阻塞当前 OS 线程！
    //   在 async 函数中调用会冻结执行器线程，使该线程上的所有其他任务停滞。
    std::thread::sleep(std::time::Duration::from_secs(5)); // 阻塞整个线程！
}

// ✅ 卸载阻塞式工作：
async fn good() {
    // → tokio::task::spawn_blocking：将同步闭包调度到**独立的阻塞线程池**执行，
    //   不占用 async 执行器线程。返回 JoinHandle<T>，可 .await 取回结果。
    //   适用于 CPU 密集、同步 I/O、调用阻塞 C 库等场景。
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
    // → 三个 task::spawn 立即返回 JoinHandle，任务在运行时上并发调度。
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
    // → tokio::join!(h1, h2, h3)：三个 handle 同时被轮询，
    //   返回三元组；总耗时约等于最慢任务（200ms）而非三者之和（450ms）。
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
// → tokio::time::timeout：为 future 套一层截止时间。
//   签名：fn timeout<T>(dur, future: T) -> Timeout<T>。
//   其 Output = Result<T::Output, Elapsed>。
use tokio::time::{timeout, Duration};

async fn fetch_with_timeout() -> Result<String, Box<dyn std::error::Error>> {
    // → timeout(dur, future).await：
    //   若 future 在 dur 内完成返回 Ok(future 的结果)；
    //   若超时则返回 Err(Elapsed)，且 future 被 drop（取消）。
    let result = timeout(Duration::from_secs(5), async {
        // 模拟慢速网络调用
        tokio::time::sleep(Duration::from_millis(100)).await;
        // → Ok::<_, E>(val)：显式标注错误类型，便于 ? 在外部统一错误类型。
        Ok::<_, Box<dyn std::error::Error>>("data".to_string())
    }).await??; // 第一个 ? 解包 Elapsed，第二个 ? 解包内部 Result

    Ok(result)
}
```

### `Send` 约束与为何 Future 必须是 `Send`

当你 `tokio::spawn` 一个 future 时，它可能在不同的 OS 线程上恢复。
这意味着 future 必须是 `Send`。常见陷阱：

```rust,ignore
// → std::rc::Rc：非原子引用计数，**不是** Send（不能跨线程移动）。
use std::rc::Rc;

async fn not_send() {
    // → Rc::new(42) 是 !Send，跨 .await 持有 rc 会使整个 Future 变 !Send。
    let rc = Rc::new(42); // Rc 是 !Send
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    println!("{}", rc); // rc 跨 .await 持有 — future 是 !Send
}

// 修复 1：在 .await 前丢弃
async fn fixed_drop() {
    let data = {
        let rc = Rc::new(42);
        *rc // 将值拷贝出来
    }; // rc 在此丢弃（局部作用域结束 drop）
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    // → data 是 i32，是 Send，故 Future 是 Send，可被 tokio::spawn。
    println!("{}", data); // 只是一个 i32，是 Send
}

// 修复 2：用 Arc 代替 Rc
async fn fixed_arc() {
    // → std::sync::Arc：原子引用计数，**是** Send + Sync（当 T: Send+Sync 时），
    //   可跨线程共享所有权，是 Rc 的线程安全版本。
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
// → 导入 tokio 异步时间 API：sleep（异步休眠）、timeout（截止时间）、Duration。
use tokio::time::{sleep, timeout, Duration};

// → async fn：异步函数，调用返回 Future，需 .await 才执行。
//   返回 Result<String, String> 模拟网络调用的成功/失败。
async fn fake_fetch(name: &'static str, delay_ms: u64) -> Result<String, String> {
    sleep(Duration::from_millis(delay_ms)).await;
    // → format! 宏构造字符串。
    Ok(format!("{name}: OK"))
}

async fn fetch_all() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let deadline = Duration::from_secs(5);

    // → timeout(deadline, future)：若内部 future 超时则返回 Err(Elapsed)。
    let (a, b, c) = timeout(deadline, async {
        // → tokio::spawn：派生并发任务，返回 JoinHandle<Result<String,String>>。
        let h1 = tokio::spawn(fake_fetch("svc-a", 100));
        let h2 = tokio::spawn(fake_fetch("svc-b", 200));
        let h3 = tokio::spawn(fake_fetch("svc-c", 150));
        // → tokio::try_join!：并发等待多个 Result future，
        //   任一 Err 则立即短路返回该错误（区别于 join! 总等全部）。
        tokio::try_join!(h1, h2, h3)
    })
    // → 第一个 ? 解包 timeout 的 Elapsed；第二个 ? 解包 try_join! 的错误。
    .await??;

    // → a?, b?, c?：解包 JoinHandle 的 Result（任务 panic 时为 Err），
    //   再解包内部的 fake_fetch Result —— 注意双重解包。
    Ok(vec![a?, b?, c?])
}

#[tokio::main]
async fn main() {
    // → fetch_all().await 返回 Result，unwrap() 在示例中直接断言成功。
    let results = fetch_all().await.unwrap();
    for r in &results {
        println!("{r}");
    }
}
```

</details>

***
