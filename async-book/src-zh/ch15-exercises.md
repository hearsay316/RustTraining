## 练习

### 练习 1：异步（async）Echo 服务器

构建一个同时处理多个客户端的 TCP 回显服务器。

**要求**：
- 监听 `127.0.0.1:8080`
- 接受连接并回显每一行
- 优雅地处理客户端断开连接
- 在客户端连接/断开时打印日志

<details>
<summary>🔑 参考答案</summary>

```rust
// ============================================================================
// 异步 Echo 服务器 — 核心概念
// ============================================================================
// 本练习演示 tokio 最基本的并发模型：每个 TCP 连接对应一个独立的任务。
//
// 关键 API：
//   TcpListener::bind().await  — 异步绑定端口，不阻塞线程
//   listener.accept().await    — 异步等待新连接到达
//   tokio::spawn(async {})     — 将连接处理逻辑提交到运行时（runtime），与其它连接并发执行
//   socket.into_split()        — 将读写半部分离，避免借用冲突
//   BufReader::new(reader)     — 按行缓冲读取，减少系统调用
//   reader.read_line().await   — 异步读取一行，等待数据到达期间挂起任务
//   writer.write_all().await   — 异步写回数据
//
// 设计理由：每个连接 spawn 一个任务是最直观的并发模型。tokio 的任务是轻量级的
// （每个仅占用几 KB 栈空间），因此可以轻松支撑数千个并发连接。

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 绑定到本地地址，await 确保绑定完成后再继续
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Echo server listening on :8080");

    loop {
        // accept() 返回 (TcpStream, SocketAddr)
        // 当没有新连接时，该 Future 处于 Pending 状态，释放当前线程
        let (socket, addr) = listener.accept().await?;
        println!("[{addr}] Connected");

        // spawn 将闭包的 Future 提交给 tokio 运行时
        // async move 将 socket 和 addr 的所有权移入新任务
        // 该任务与 accept 循环并发运行，互不阻塞
        tokio::spawn(async move {
            // into_split() 将 TcpStream 分为独立的读半部和写半部
            // 这是必要的，因为后续需要同时持有 reader 和 writer
            let (reader, mut writer) = socket.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear(); // 每次复用 String，避免重复分配内存
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // read_line 返回 0 表示对端已关闭连接（EOF）
                        println!("[{addr}] Disconnected");
                        break;
                    }
                    Ok(_) => {
                        print!("[{addr}] Echo: {line}");
                        // write_all 确保完整写入，处理部分写入的情况
                        if writer.write_all(line.as_bytes()).await.is_err() {
                            println!("[{addr}] Write error, disconnecting");
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[{addr}] Read error: {e}");
                        break;
                    }
                }
            }
        });
    }
}
```

</details>

---

### 练习 2：带并发限制的 URL 抓取器

并发抓取 URL 列表，最多 5 个并发请求。

<details>
<summary>🔑 参考答案</summary>

```rust
// ============================================================================
// 带并发限制的 URL 抓取器 — 核心概念
// ============================================================================
// 本练习演示如何使用 Stream 来控制并发数量，而非手动管理 Semaphore。
//
// 关键 API：
//   stream::iter(urls)        — 将 Vec 转换为 Stream，惰性产生元素
//   .map(|url| async {})      — 将每个 URL 映射为一个异步闭包（返回 Future）
//   .buffer_unordered(5)      — 同时轮询最多 5 个 Future，任意一个完成即产出结果
//   .collect().await           — 消费 Stream，收集所有结果到 Vec
//
// 设计理由：buffer_unordered 是限制并发的声明式方案。它内部维护一个最多 N 个
// Future 的缓冲区，当某个 Future 完成后，立即从上游拉取下一个元素填充缓冲区。
// 这比手动 Semaphore::acquire 更简洁，且避免了忘记释放许可的 bug。
//
// 选择依据：
//   buffer_unordered → 适用于 Stream（有序迭代器产生 Future）
//   Semaphore        → 适用于独立 spawn 的任务（无法组织成 Stream）
//   不要将两者混用来实现同一限制目标。

use futures::stream::{self, StreamExt};

async fn fetch_urls(urls: Vec<String>) -> Vec<Result<String, String>> {
    let results: Vec<_> = stream::iter(urls)
        .map(|url| {
            async move {
                println!("Fetching: {url}");

                // reqwest::get 返回 Result<Response, Error>
                // 两层 Result：外层是网络错误，内层是读取响应体错误
                match reqwest::get(&url).await {
                    Ok(resp) => match resp.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("{url}: {e}")),
                    },
                    Err(e) => Err(format!("{url}: {e}")),
                }
            }
        })
        .buffer_unordered(5) // ← 核心：最多 5 个并发请求同时进行
        .collect()           // 等待所有 Future 完成后收集结果
        .await;

    results
}
```

</details>

---

### 练习 3：带工作池的优雅关闭（graceful shutdown）

构建一个具备以下特性的任务处理器：
- 基于 channel 的工作队列
- N 个 worker 从队列中消费任务
- 按 Ctrl+C 触发优雅关闭：停止接受新任务，完成进行中的工作后退出

<details>
<summary>🔑 参考答案</summary>

```rust
// ============================================================================
// 带工作池的优雅关闭 — 核心概念
// ============================================================================
// 本练习演示两个关键生产模式：(1) mpsc 工作队列 (2) watch channel 优雅关闭。
//
// 关键 API：
//   mpsc::channel(100)   — 多生产者单消费者通道，缓冲区容量 100
//   watch::channel(false) — 单生产者多消费者，只保留最新值
//   tokio::select!        — 同时等待多个异步操作，任一就绪即执行
//   shutdown.changed()    — 等待 watch 值变更的通知
//   producer.abort()      — 强制取消生产者任务
//
// 架构设计：
//   ┌──────────┐    mpsc     ┌──────────┐
//   │ Producer │ ──────────> │ Worker 0 │
//   │  (tx)    │             │ Worker 1 │
//   └──────────┘             │ Worker 2 │
//        │                   │ Worker 3 │
//        │ watch             └──────────┘
//        ▼                        ▲
//   ┌──────────┐                  │
//   │ Shutdown │──────────────────┘
//   │  (tx)    │   watch clone
//   └──────────┘
//
// 关闭流程：
//   1. Ctrl+C 触发 shutdown_tx.send(true)
//   2. 每个 Worker 的 select! 检测到 shutdown.changed()
//   3. Producer 被 abort() 强制取消
//   4. Worker 完成当前任务后退出（drop mpsc::Receiver 使 recv() 返回 None）

use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};

struct WorkItem {
    id: u64,
    payload: String,
}

#[tokio::main]
async fn main() {
    // mpsc channel: Producer 发送，Worker 接收
    let (work_tx, work_rx) = mpsc::channel::<WorkItem>(100);
    // watch channel: 广播关闭信号给所有 Worker
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // 生成 4 个 Worker
    let mut worker_handles = Vec::new();
    // Arc<Mutex> 包装 Receiver 以支持多 Worker 共享
    // 注意：这里用的是 tokio::sync::Mutex，因为锁需要在 .await 之间持有
    let work_rx = std::sync::Arc::new(tokio::sync::Mutex::new(work_rx));

    for id in 0..4 {
        let rx = work_rx.clone();
        let mut shutdown = shutdown_rx.clone(); // 每个 Worker 独立订阅关闭信号
        let handle = tokio::spawn(async move {
            loop {
                // select! 同时等待两个事件：
                //   1. 新任务到达（rx.recv()）
                //   2. 关闭信号（shutdown.changed()）
                let item = {
                    let mut rx = rx.lock().await; // 获取锁后才能调用 recv()
                    tokio::select! {
                        item = rx.recv() => item,
                        _ = shutdown.changed() => {
                            // changed() 返回后检查当前值
                            if *shutdown.borrow() { None } else { continue }
                        }
                    }
                };

                match item {
                    Some(work) => {
                        println!("Worker {id}: processing item {}", work.id);
                        sleep(Duration::from_millis(200)).await; // 模拟 I/O 工作
                        println!("Worker {id}: done with item {}", work.id);
                    }
                    None => {
                        // recv() 返回 None 表示所有 Sender 已 drop，channel 关闭
                        println!("Worker {id}: channel closed, exiting");
                        break;
                    }
                }
            }
        });
        worker_handles.push(handle);
    }

    // Producer：提交 20 个工作项，每个间隔 50ms 模拟流入速率
    let producer = tokio::spawn(async move {
        for i in 0..20 {
            let _ = work_tx.send(WorkItem {
                id: i,
                payload: format!("task-{i}"),
            }).await;
            sleep(Duration::from_millis(50)).await;
        }
    });

    // 等待 Ctrl+C 信号（跨平台，Unix 为 SIGINT，Windows 为 Ctrl+C）
    tokio::signal::ctrl_c().await.unwrap();
    println!("\nShutdown signal received!");

    // 发送关闭信号：watch::send 会通知所有订阅者
    shutdown_tx.send(true).unwrap();

    // 强制取消 Producer（防止它继续发送新任务）
    producer.abort();

    // 等待所有 Worker 完成当前任务并退出
    for handle in worker_handles {
        let _ = handle.await;
    }
    println!("All workers shut down. Goodbye!");
}
```

</details>

---

### 练习 4：从头构建一个简易异步 Mutex

使用 channel 实现异步感知的互斥体（不使用 `tokio::sync::Mutex`）。

*提示*：使用带 1 个许可的 `tokio::sync::Semaphore` 来序列化访问。

<details>
<summary>🔑 参考答案</summary>

```rust
// ============================================================================
// 简易异步 Mutex 实现 — 核心概念
// ============================================================================
// 本练习揭示异步 Mutex 的内部机制：用 Semaphore 的许可管理替代阻塞式锁。
//
// 关键 API 与类型：
//   UnsafeCell<T>      — 绕过 Rust 借用检查的原始内存容器（外部保证安全性）
//   Semaphore::new(1)  — 只有 1 个许可，相当于"锁"
//   acquire_owned()    — 异步获取许可，返回 OwnedSemaphorePermit
//   OwnedSemaphorePermit — RAII：drop 时自动释放许可
//
// 设计原理：
//   1. Semaphore(1) 保证同时只有一个任务持有许可 → 互斥访问
//   2. acquire_owned() 是异步的 → 等待锁时不阻塞线程
//   3. OwnedSemaphorePermit 包装在 SimpleGuard 中 → drop guard 即释放锁
//   4. UnsafeCell 提供内部可变性 → 绕过借用检查，由程序员保证并发安全
//
// 为什么不用 std::sync::Mutex？
//   std::sync::Mutex::lock() 是阻塞调用，会阻塞整个线程。
//   在异步上下文中，阻塞线程意味着该线程上的所有其它任务也被阻塞。
//   异步 Mutex 通过 Semaphore 将"等待"转换为任务挂起而非线程阻塞。

use std::cell::UnsafeCell;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct SimpleAsyncMutex<T> {
    data: Arc<UnsafeCell<T>>,       // 被保护的数据（Arc 支持多线程共享）
    semaphore: Arc<Semaphore>,      // 信号量实现互斥（容量为 1）
}

// SAFETY: 对 T 的访问由信号量序列化（同一时刻最多 1 个许可），
// 因此跨线程 Send 和 Sync 是安全的。
unsafe impl<T: Send> Send for SimpleAsyncMutex<T> {}
unsafe impl<T: Send> Sync for SimpleAsyncMutex<T> {}

// SimpleGuard 是 RAII guard：持有期间独占访问，drop 时释放锁
pub struct SimpleGuard<T> {
    data: Arc<UnsafeCell<T>>,
    _permit: OwnedSemaphorePermit, // 当 guard 被 drop 时，许可自动归还信号量
}

impl<T> SimpleAsyncMutex<T> {
    pub fn new(value: T) -> Self {
        SimpleAsyncMutex {
            data: Arc::new(UnsafeCell::new(value)),
            semaphore: Arc::new(Semaphore::new(1)), // 1 = 互斥
        }
    }

    /// 异步获取锁。如果锁被持有，当前任务挂起直到许可可用。
    pub async fn lock(&self) -> SimpleGuard<T> {
        // acquire_owned() 返回 Future，.await 期间任务挂起而非线程阻塞
        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        SimpleGuard {
            data: self.data.clone(),
            _permit: permit, // RAII: guard 存活 = 锁被持有
        }
    }
}

// Deref：允许通过 guard 不可变访问内部数据
impl<T> std::ops::Deref for SimpleGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY：我们持有唯一的信号量许可，保证没有其他 guard 同时存在，
        // 因此对 UnsafeCell 的不可变引用是安全的。
        unsafe { &*self.data.get() }
    }
}

// DerefMut：允许通过 guard 可变访问内部数据
impl<T> std::ops::DerefMut for SimpleGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY：同样的理由——单一许可保证独占访问，可变引用是安全的。
        unsafe { &mut *self.data.get() }
    }
}

// 当 SimpleGuard 离开作用域时，_permit 被 drop，
// 信号量许可自动归还，等待中的 lock() 调用可以继续执行。

// 用法示例：
// let mutex = SimpleAsyncMutex::new(vec![1, 2, 3]);
// {
//     let mut guard = mutex.lock().await;  // 异步等待锁
//     guard.push(4);                        // 通过 DerefMut 修改内部数据
// } // guard 在此处 drop，许可释放
```

**关键要点**：异步 Mutex 通常构建在信号量之上。信号量提供了异步等待机制——锁定时，`acquire()` 挂起任务而非阻塞线程，直到许可被释放。这正是 `tokio::sync::Mutex` 的内部工作原理。

> **为什么用 `UnsafeCell` 而不是 `std::sync::Mutex`？** 本练习的早期版本使用了 `Arc<Mutex<T>>` 配合 `Deref`/`DerefMut` 调用 `.lock().unwrap()`。这无法编译——返回的 `&T` 借用了临时的 `MutexGuard`，而该 guard 会被立即 drop。`UnsafeCell` 避免了中间 guard，基于信号量的序列化让 `unsafe` 的使用有声可循。

</details>

---

### 练习 5：Stream 管道

使用 Stream 构建数据处理管道：
1. 生成数字 1..=100
2. 过滤出偶数
3. 将每个数字映射为其平方
4. 每次并发处理 10 个（用 sleep 模拟）
5. 收集结果

<details>
<summary>🔑 参考答案</summary>

```rust
// ============================================================================
// Stream 管道 — 核心概念
// ============================================================================
// 本练习演示 Stream 的声明式链式处理，类似于 Iterator 的 .map/.filter，
// 但每一步可以涉及异步操作。
//
// 关键 API：
//   stream::iter(1..=100)      — 从 Range 创建 Stream（惰性）
//   .filter(|x| ready(cond))   — 同步过滤，ready() 将 bool 包装为立即就绪的 Future
//   .map(|x| x * x)            — 同步映射
//   .map(|x| async { ... })    — 异步映射，将每个元素转为异步计算
//   .buffer_unordered(10)      — 同时驱动最多 10 个异步映射的结果
//   .collect().await            — 等待所有结果就绪，收集到 Vec
//
// 管道数据流：
//   1..=100 → filter(偶数) → map(平方) → async_map(sleep+打印) → buffer(10) → collect
//
// buffer_unordered 的行为：
//   - 上游产生 50 个偶数 Future
//   - 同时 poll 最多 10 个
//   - 某 Future 完成后，立即从上游拉取下一个
//   - 结果以完成顺序产出（非输入顺序）

use futures::stream::{self, StreamExt};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let results: Vec<u64> = stream::iter(1u64..=100)
        // 第 2 步：过滤 — 只保留偶数
        // future::ready() 创建立即就绪的 Future，满足 filter 的异步签名要求
        .filter(|x| futures::future::ready(x % 2 == 0))
        // 第 3 步：映射 — 计算平方（同步操作，无需 .await）
        .map(|x| x * x)
        // 第 4 步：异步处理 — 模拟 I/O 工作（如写数据库、调 API）
        .map(|x| async move {
            sleep(Duration::from_millis(50)).await; // 模拟 50ms 的 I/O 延迟
            println!("Processed: {x}");
            x // 异步闭包返回处理后的值
        })
        .buffer_unordered(10) // 核心：10 个并发上限，控制资源使用
        // 第 5 步：收集 — 等待所有 50 个偶数处理完成
        .collect()
        .await;

    println!("Got {} results", results.len());
    println!("Sum: {}", results.iter().sum::<u64>());
}
```

</details>

---

### 练习 6：实现带超时的 Select

在不使用 `tokio::select!` 或 `tokio::time::timeout` 的情况下，实现一个函数，使其与截止时间竞争 Future，当 Future 先完成时返回 `Either::Left(result)`，超时时返回 `Either::Right(())`。

*提示*：基于第 6 章中的 `Select` 组合器和同一章中的 `TimerFuture` 进行构建。

<details>
<summary>🔑 参考答案</summary>

```rust,ignore
// ============================================================================
// 带超时的 Select 实现 — 核心概念
// ============================================================================
// 本练习展示 timeout/select 的底层原理：poll 两个 Future，返回先完成者的结果。
//
// 关键 API：
//   Pin<&mut Self>           — 固定引用，确保自引用 Future 不会被移动
//   Pin::new(&mut field)     — 从 Pin 投影到内部字段（仅适用于 Unpin 字段）
//   cx: &mut Context<'_>     — 携带 Waker，通知执行器（executor）可以重新 poll
//   Poll::Ready(val)         — Future 已完成，返回值
//   Poll::Pending            — Future 未就绪，已注册 Waker
//
// 执行流程（每次 poll）：
//   1. poll 主 Future → 如果就绪，返回 Either::Left(result)
//   2. poll 定时器  → 如果超时，返回 Either::Right(())
//   3. 两者都未就绪 → 返回 Pending（两个 Future 都已注册 Waker）
//
// 这与 tokio::select! 的原理完全相同：
//   每次 poll 时检查所有分支，任一就绪即返回，否则注册 Waker 等待唤醒。

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

// Either 枚举：左值 = Future 结果，右值 = 超时
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

// Timeout 组合器：包装一个 Future 和一个 TimerFuture
pub struct Timeout<F> {
    future: F,
    timer: TimerFuture, // 来自第 6 章的定时器实现
}

impl<F: Future + Unpin> Timeout<F> {
    pub fn new(future: F, duration: Duration) -> Self {
        Timeout {
            future,
            timer: TimerFuture::new(duration), // 创建指定时长的定时器
        }
    }
}

// 为 Timeout 实现 Future trait，使其本身可以被 .await
impl<F: Future + Unpin> Future for Timeout<F> {
    type Output = Either<F::Output, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 检查主 Future 是否已完成
        // Pin::new 用于从 Pin<&mut Self> 投影到具体字段
        if let Poll::Ready(val) = Pin::new(&mut self.future).poll(cx) {
            return Poll::Ready(Either::Left(val));
        }

        // 检查定时器是否已到期
        if let Poll::Ready(()) = Pin::new(&mut self.timer).poll(cx) {
            return Poll::Ready(Either::Right(()));
        }

        // 两者都未就绪 → Pending。两个 Future 内部都已注册 Waker，
        // 任意一个就绪时执行器会重新 poll 此 Timeout Future。
        Poll::Pending
    }
}

// 用法示例：
// match Timeout::new(fetch_data(), Duration::from_secs(5)).await {
//     Either::Left(data) => println!("Got data: {data}"),
//     Either::Right(()) => println!("Timed out!"),
// }
```

**关键要点**：`select`/`timeout` 的本质就是轮询两个 Future 并看哪个先完成。整个异步生态系统都建立在这个简单的原语之上：poll、Pending/Ready、Waker。

</details>

***

