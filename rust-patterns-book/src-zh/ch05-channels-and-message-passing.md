# 5. 通道与消息传递 🟢

> **你将学到：**
> - `std::sync::mpsc` 基础以及何时升级到 crossbeam-channel
> - 使用 `select!` 进行多来源消息的通道选择
> - 有界与无界通道以及背压（backpressure）策略
> - 用于封装并发状态的 actor 模式

## std::sync::mpsc —— 标准通道

Rust 标准库提供了一个多生产者、单消费者（multi-producer, single-consumer）通道：

```rust
// ===========================================================
// 核心概念：mpsc = 多生产者（multi-producer）单消费者（single-consumer）通道
// ===========================================================
// 这段代码演示了标准库通道的基本用法：
//   1. channel()  → 创建一对 (Sender, Receiver)，无界缓冲
//   2. tx.clone() → 克隆发送端，实现"多生产者"
//   3. tx.send()  → 发送消息（无界通道永不阻塞）
//   4. for msg in rx → Receiver 实现了 Iterator，通道关闭时迭代结束

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn main() {
    // ↓ channel() 创建一个无界异步通道，返回元组 (Sender<T>, Receiver<T>)
    // → 签名：pub fn channel<T>() -> (Sender<T>, Receiver<T>)
    //   tx: 发送端，可 Clone（多生产者）；rx: 接收端，不可 Clone（单消费者）
    let (tx, rx) = mpsc::channel();

    // ↓ tx.clone() 克隆一个 Sender，使两个线程可以各自持有一个发送端
    // → Sender 实现了 Clone trait；内部用 Arc 共享底层队列
    // → 克隆是廉价的（仅复制 Arc 指针）
    let tx1 = tx.clone(); // 为多生产者克隆发送端

    // ↓ thread::spawn 在新 OS 线程中运行闭包，返回 JoinHandle
    // → 签名：pub fn spawn<F, T>(f: F) -> JoinHandle<T>
    //   要求 F: FnOnce() -> T + Send + 'static
    //   move 关键字将 tx1 的所有权转移到闭包（满足 'static）
    thread::spawn(move || {
        for i in 0..5 {
            // ↓ send() 将值 T 通过通道发送给接收端
            // → 签名：pub fn send(&self, t: T) -> Result<(), SendError<T>>
            //   无界通道永不阻塞（堆内存增长）；接收端被丢弃时返回 Err
            //   unwrap() 在出错时 panic —— 生产代码应处理 SendError
            tx1.send(format!("producer-1: msg {i}")).unwrap();
            // → sleep 让当前线程休眠指定时长，模拟工作负载
            thread::sleep(Duration::from_millis(100));
        }
    });
    // 注意：tx1 在此闭包结束时被 drop —— 一个 Sender 消失

    // 第二个生产者：move 把原始 tx 转移进去（不再可用）
    thread::spawn(move || {
        for i in 0..5 {
            tx.send(format!("producer-2: msg {i}")).unwrap();
            thread::sleep(Duration::from_millis(150));
        }
    });
    // 注意：现在原始 tx 也被 drop —— 所有 Sender 都消失了

    // ↓ Receiver 实现了 IntoIterator，for 循环每次取一条消息
    // → 迭代在"所有 Sender 都被 drop 且队列清空"时自然结束
    // → 这是通道关闭的标准信号：不再有任何生产者
    for msg in rx {
        println!("Received: {msg}");
    }
    println!("All producers done.");
}
```

> **注意：** 在 `.send()` 上使用 `.unwrap()` 是为了简洁。如果接收者已被丢弃，它会 panic。生产代码应优雅地处理 `SendError`。

**关键特性**：
- 默认**无界**（如果消费者很慢，可能填满内存）
- `mpsc::sync_channel(N)` 创建带背压的**有界**通道
- `rx.recv()` 会阻塞当前线程直到有消息到达
- `rx.try_recv()` 立即返回，如果没有就绪的消息则返回 `Err(TryRecvError::Empty)`
- 当所有 `Sender` 都被丢弃时通道关闭

```rust
// ===========================================================
// 有界通道：通过固定容量的缓冲区实现背压（backpressure）
// ===========================================================
// 当消费者跟不上生产者时，有界通道会阻塞发送方，
// 从而防止内存被无限增长的消息队列耗尽（OOM）。

// ↓ sync_channel(N) 创建一个容量为 N 的有界通道
// → 签名：pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>)
//   SyncSender::send 在缓冲区满时会阻塞当前线程（背压！）
//   bound = 0 表示会合通道（rendezvous）：send 阻塞直到 recv 就绪
let (tx, rx) = mpsc::sync_channel(10); // 缓冲区容量为 10

thread::spawn(move || {
    for i in 0..1000 {
        // ↓ 当缓冲区已满（已有 10 条未消费消息）时，send 会阻塞
        // → 这就是"背压"：生产者被迫等待消费者，形成天然的限流机制
        // → 签名：pub fn send(&self, t: T) -> Result<(), SendError<T>>
        tx.send(i).unwrap(); // 如果缓冲区已满则阻塞 —— 自然的背压
    }
});
```

> **注意：** 使用 `.unwrap()` 是为了简洁。在生产代码中，应处理 `SendError`（接收者已丢弃）而非 panic。

### crossbeam-channel —— 生产环境的主力

`crossbeam-channel` 是生产环境通道使用的事实标准。它比 `std::sync::mpsc` 更快，并支持多消费者（`mpmc`）：

```rust,ignore
// ===========================================================
// crossbeam-channel：支持多生产者多消费者（MPMC）的高性能通道
// ===========================================================
// 与 std::sync::mpsc 的关键区别：
//   1. Receiver 可以 Clone —— 真正的多消费者（工作窃取模式）
//   2. 性能更高（基于经过优化无锁算法）
//   3. 提供 select! 宏进行多路通道选择

// Cargo.toml:
//   [dependencies]
//   crossbeam-channel = "0.5"
use crossbeam_channel::{bounded, unbounded, select, Sender, Receiver};
use std::thread;
use std::time::Duration;

fn main() {
    // ↓ bounded::<T>(n) 创建一个容量为 n 的有界 MPMC 通道
    // → 签名：pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>)
    //   Sender 和 Receiver 都实现了 Clone —— 这是 MPMC 的关键
    let (tx, rx) = bounded::<String>(100);

    // 多个生产者：每个线程克隆一份 tx
    for id in 0..4 {
        let tx = tx.clone(); // → Sender::clone 廉价，共享底层队列
        thread::spawn(move || {
            for i in 0..10 {
                // → send 在缓冲区满时阻塞；返回 ()（不会因接收端消失而 Err）
                tx.send(format!("worker-{id}: item-{i}")).unwrap();
            }
        });
    }
    // ↓ drop 掉原始 tx：当所有克隆也被 drop 后，通道才关闭
    // → 这是让消费者 for/while 循环正常退出的关键
    drop(tx); // 丢弃原始发送端，以便通道可以关闭

    // ↓ rx.clone() —— crossbeam 允许多个消费者！std::sync::mpsc 做不到
    // → 多个 Receiver 共享同一队列；每条消息只会被其中一个消费者取走
    let rx2 = rx.clone();
    let consumer1 = thread::spawn(move || {
        // ↓ recv() 阻塞等待下一条消息
        // → 签名：pub fn recv(&self) -> Result<T, RecvError>
        //   通道关闭（所有 Sender 被 drop 且队列为空）时返回 Err
        while let Ok(msg) = rx.recv() {
            println!("[consumer-1] {msg}");
        }
    });
    let consumer2 = thread::spawn(move || {
        while let Ok(msg) = rx2.recv() {
            println!("[consumer-2] {msg}");
        }
    });

    // ↓ join() 阻塞当前线程直到该线程结束
    // → 签名：pub fn join(self) -> Result<T, Box<dyn Any + Send>>
    //   线程 panic 时返回 Err；unwrap 在此时会再次 panic
    consumer1.join().unwrap();
    consumer2.join().unwrap();
}
```

### 通道选择（select!）

同时监听多个通道——类似于 Go 中的 `select`：

```rust,ignore
// ===========================================================
// select! 宏：同时监听多个通道，谁先就绪就处理谁
// ===========================================================
// 类似 Go 的 select 语句，crossbeam 的 select! 会：
//   1. 随机化就绪分支的选择顺序（防止饥饿）
//   2. 阻塞直到至少一个分支就绪
//   3. 常用于：工作通道 + 心跳定时器 + 超时退出的组合

use crossbeam_channel::{bounded, tick, after, select};
use std::time::Duration;

fn main() {
    let (work_tx, work_rx) = bounded::<String>(10);
    // ↓ tick(d) 创建一个周期性触发器，每隔 d 发送一个 Instant
    // → 常用于心跳/定期轮询
    let ticker = tick(Duration::from_secs(1));        // 周期性心跳
    // ↓ after(d) 创建一个一次性超时器，d 后发送一个 Instant
    // → 常用于总超时控制
    let deadline = after(Duration::from_secs(10));     // 一次性超时

    // 生产者
    let tx = work_tx.clone();
    std::thread::spawn(move || {
        for i in 0..100 {
            tx.send(format!("job-{i}")).unwrap();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    drop(work_tx);

    loop {
        // ↓ select! 宏：同时等待多个 recv 操作
        // → 语法：recv(channel) -> msg => { ... }
        //   msg 类型为 Result<T, RecvError>，Err 表示通道关闭
        select! {
            recv(work_rx) -> msg => {
                match msg {
                    Ok(job) => println!("Processing: {job}"),
                    Err(_) => {
                        println!("Work channel closed");
                        break;
                    }
                }
            },
            recv(ticker) -> _ => {
                println!("Tick — heartbeat");
            },
            recv(deadline) -> _ => {
                println!("Deadline reached — shutting down");
                break;
            },
        }
    }
}
```

> **与 Go 对比**：这完全类似于 Go 中对通道的 `select` 语句。
> crossbeam 的 `select!` 宏会随机化顺序以防止饥饿，就像 Go 一样。

### 有界 vs 无界与背压

| 类型 | 满时的行为 | 内存 | 使用场景 |
|------|-------------------|--------|----------|
| **无界（Unbounded）** | 永不阻塞（堆增长） | 无界 ⚠️ | 罕见——仅当生产者比消费者慢时 |
| **有界（Bounded）** | `send()` 阻塞直到有空间 | 固定 | 生产环境默认——防止 OOM |
| **会合（Rendezvous）**（bounded(0)） | `send()` 阻塞直到接收者就绪 | 无 | 同步 / 交接 |

```rust
// ===========================================================
// 会合通道（rendezvous channel）：零容量，直接交接
// ===========================================================
// bounded(0) 创建的通道没有缓冲区：
//   send 会阻塞，直到 recv 也被调用 —— 两端"会合"
//   这是最强的同步原语：发送方与接收方完全同步

// ↓ bounded(0) → 容量为 0 的有界通道
// → 等价于 Go 的无缓冲 channel
let (tx, rx) = crossbeam_channel::bounded(0);
// tx.send(x) 阻塞直到 rx.recv() 被调用，反之亦然。
// 这精确地同步了两个线程。
```

**规则**：在生产环境中始终使用有界通道，除非你能证明生产者永远不会超过消费者。

### 使用通道的 Actor 模式

Actor 模式使用通道来串行化对可变状态的访问——不需要互斥锁（mutex）：

```rust
// ===========================================================
// Actor 模式：用通道串行化对可变状态的访问，无需互斥锁
// ===========================================================
// 核心思想：状态只存在于一个线程（actor）内部，外部通过发消息来操作它。
//   - 外部拥有一个轻量的 Counter 句柄（仅一个 Sender），可被 Clone
//   - actor 线程独占 count，串行处理所有消息 —— 天然线程安全
//   - 查询操作通过"回复通道"（reply channel）返回结果

use std::sync::mpsc;
use std::thread;

// Actor 能接收的消息类型（命令模式）
enum CounterMsg {
    Increment,
    Decrement,
    // ↓ 携带一个回复通道：actor 处理后通过它回传 i64
    // → 这种"在消息中嵌入回复通道"的模式是 actor 请求/响应的标准做法
    Get(mpsc::Sender<i64>), // 回复通道
}

struct CounterActor {
    count: i64,
    rx: mpsc::Receiver<CounterMsg>,
}

impl CounterActor {
    fn new(rx: mpsc::Receiver<CounterMsg>) -> Self {
        CounterActor { count: 0, rx }
    }

    // ↓ run 消费 self，进入消息循环，直到通道关闭（所有 Sender 被 drop）
    fn run(mut self) {
        // ↓ recv() 阻塞等待下一条消息；返回 Err 时循环结束
        // → 通道关闭 = 所有 Counter 句柄被 drop = actor 该退出了
        while let Ok(msg) = self.rx.recv() {
            match msg {
                CounterMsg::Increment => self.count += 1,
                CounterMsg::Decrement => self.count -= 1,
                CounterMsg::Get(reply) => {
                    // ↓ reply.send 通过嵌入的回复通道回传当前计数值
                    // → 忽略返回值：调用方可能在发送后立即放弃等待
                    let _ = reply.send(self.count);
                }
            }
        }
    }
}

// Actor 句柄 —— 廉价克隆，Send + Sync（因为它只持有一个 Sender）
#[derive(Clone)]
struct Counter {
    tx: mpsc::Sender<CounterMsg>,
}

impl Counter {
    // ↓ spawn 创建 actor 线程并返回句柄
    fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        // → 在新线程中运行 actor，rx 的所有权被 move 进去
        thread::spawn(move || CounterActor::new(rx).run());
        Counter { tx }
    }

    // ↓ 增/减操作：只是发一条消息，立即返回（fire-and-forget）
    fn increment(&self) { let _ = self.tx.send(CounterMsg::Increment); }
    fn decrement(&self) { let _ = self.tx.send(CounterMsg::Decrement); }

    // ↓ get 是同步请求/响应：发送查询，阻塞等待回复
    fn get(&self) -> i64 {
        // ↓ 临时创建一条"回复通道"用于这次查询
        let (reply_tx, reply_rx) = mpsc::channel();
        // → 把 reply_tx 随消息一起发给 actor
        self.tx.send(CounterMsg::Get(reply_tx)).unwrap();
        // ↓ recv 阻塞等待 actor 通过回复通道回传结果
        // → 由于 actor 串行处理，回复值一定是最新状态
        reply_rx.recv().unwrap()
    }
}

fn main() {
    let counter = Counter::spawn();

    // ↓ 多个线程可以安全地使用 counter —— 无需 mutex！
    // → 因为真正的状态只存在于 actor 线程内
    let handles: Vec<_> = (0..10).map(|_| {
        let counter = counter.clone(); // → 句柄克隆，廉价
        thread::spawn(move || {
            for _ in 0..1000 {
                counter.increment();
            }
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
    println!("Final count: {}", counter.get()); // 10000
}
```

> **何时使用 actor vs 互斥锁**：当状态有复杂的不变式、操作耗时较长，
> 或者你想串行化访问而无需考虑锁顺序时，actor 很合适。对于短临界区，互斥锁更简单。

> **核心要点 —— 通道**
> - `crossbeam-channel` 是生产环境的主力——比 `std::sync::mpsc` 更快、功能更丰富
> - `select!` 用声明式通道选择替代了复杂的多来源轮询
> - 有界通道提供自然的背压；无界通道有 OOM 风险

> **参见：** [第 6 章 —— 并发](ch06-concurrency-vs-parallelism-vs-threads.md) 了解线程、Mutex 和共享状态。[第 15 章 —— Async](ch16-asyncawait-essentials.md) 了解异步通道（`tokio::sync::mpsc`）。

---

### 练习：基于通道的工作线程池 ★★★（约 45 分钟）

使用通道构建一个工作线程池，其中：
- 分发器（dispatcher）通过通道发送 `Job` 结构体
- N 个工作线程消费任务并发回结果
- 使用 `std::sync::mpsc` 配合 `Arc<Mutex<Receiver>>` 实现共享工作队列

<details>
<summary>🔑 答案</summary>

```rust
// ===========================================================
// 工作线程池：分发任务 → 多 worker 并发处理 → 汇总结果
// ===========================================================
// 经典模式：一个共享的"工作队列"（Arc<Mutex<Receiver>>），
//   多个 worker 竞争抢任务，互不干扰。
// 关键点：
//   1. Arc<Mutex<Receiver>> 让多个线程共享一个 Receiver
//   2. 锁只保护"取出一条任务"这个瞬间操作，临界区极短
//   3. 两个独立通道：job 通道（派发）+ result 通道（回收）

use std::sync::mpsc;
use std::thread;

struct Job {
    id: u64,
    data: String,
}

struct JobResult {
    job_id: u64,
    output: String,
    worker_id: usize,
}

fn worker_pool(jobs: Vec<Job>, num_workers: usize) -> Vec<JobResult> {
    // ↓ 任务通道：主线程派发，worker 消费
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    // ↓ 结果通道：worker 发送，主线程收集
    let (result_tx, result_rx) = mpsc::channel::<JobResult>();

    // ↓ Arc<Mutex<Receiver>> —— 多线程共享单一接收端
    // → Arc 提供共享所有权；Mutex 保护 recv() 不被并发调用
    // → std::sync::mpsc 的 Receiver 不支持 Clone，只能这样共享
    let job_rx = std::sync::Arc::new(std::sync::Mutex::new(job_rx));

    let mut handles = Vec::new();
    for worker_id in 0..num_workers {
        let job_rx = job_rx.clone();   // → Arc 克隆，廉价
        let result_tx = result_tx.clone();
        handles.push(thread::spawn(move || {
            loop {
                // ↓ 关键技巧：用块作用域限定锁的持有时间
                //   锁 → recv 取任务 → 立即释放锁 → 在锁外处理
                let job = {
                    // ↓ lock() 返回 MutexGuard，drop 时自动解锁
                    let rx = job_rx.lock().unwrap();
                    // → recv() 在持锁期间阻塞等待任务
                    //   注意：持锁阻塞会卡住其他 worker —— 但这是 SPSC 通道的标准写法
                    rx.recv()
                }; // ← guard 在此 drop，锁释放

                match job {
                    Ok(job) => {
                        let output = format!("processed '{}' by worker {worker_id}", job.data);
                        // → result_tx.send 发送处理结果，多生产者共享
                        result_tx.send(JobResult {
                            job_id: job.id, output, worker_id,
                        }).unwrap();
                    }
                    Err(_) => break, // → 通道关闭（job_tx 被 drop），worker 退出
                }
            }
        }));
    }
    // ↓ drop 掉 result_tx 的原始副本，这样当所有 worker 结束后
    //   result_rx 的迭代才会自然终止
    drop(result_tx);

    let num_jobs = jobs.len();
    // ↓ 派发所有任务
    for job in jobs {
        job_tx.send(job).unwrap();
    }
    // ↓ drop job_tx：所有 worker 的 recv() 会陆续返回 Err，循环退出
    drop(job_tx);

    // ↓ into_iter().collect() 消费接收端直到通道关闭
    // → result_tx 被 drop + 所有 worker 结束后，迭代停止
    let results: Vec<_> = result_rx.into_iter().collect();
    assert_eq!(results.len(), num_jobs);

    for h in handles { h.join().unwrap(); }
    results
}

fn main() {
    let jobs: Vec<Job> = (0..20).map(|i| Job {
        id: i, data: format!("task-{i}"),
    }).collect();

    let results = worker_pool(jobs, 4);
    for r in &results {
        println!("[worker {}] job {}: {}", r.worker_id, r.job_id, r.output);
    }
}
```

</details>

***
