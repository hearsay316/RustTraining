# 6. 并发 vs 并行 vs 线程 🟡

> **你将学到：**
> - 并发（concurrency）与并行（parallelism）的精确区别
> - OS 线程、作用域线程（scoped threads）和用于数据并行的 rayon
> - 共享状态原语：Arc、Mutex、RwLock、原子类型（Atomics）、Condvar
> - 使用 OnceLock/LazyLock 进行延迟初始化以及无锁模式

## 术语：并发 ≠ 并行

这些术语经常被混淆。以下是精确的区别：

| | 并发（Concurrency） | 并行（Parallelism） |
|---|---|---|
| **定义** | 管理可以取得进展的多个任务 | 同时执行多个任务 |
| **硬件要求** | 单核即可 | 需要多核 |
| **类比** | 一个厨师做多道菜（在它们之间切换） | 多个厨师，每人做一道菜 |
| **Rust 工具** | `async/await`、通道、`select!` | `rayon`、`thread::spawn`、`par_iter()` |

```text
并发（单核）：                        并行（多核）：

任务 A: ██░░██░░██                    任务 A: ██████████
任务 B: ░░██░░██░░                    任务 B: ██████████
─────────────────→ 时间               ─────────────────→ 时间
（在一个核心上交替执行）               （在两个核心上同时执行）
```

### std::thread —— OS 线程

Rust 线程与 OS 线程是 1:1 映射的。每个线程都有自己的栈（通常为 2-8 MB）：

```rust
use std::thread;
use std::time::Duration;

fn main() {
    // 生成一个线程——接受一个闭包
    let handle = thread::spawn(|| {
        for i in 0..5 {
            println!("spawned thread: {i}");
            thread::sleep(Duration::from_millis(100));
        }
        42 // 返回值
    });

    // 同时在主线程上工作
    for i in 0..3 {
        println!("main thread: {i}");
        thread::sleep(Duration::from_millis(150));
    }

    // 等待线程结束并获取其返回值
    let result = handle.join().unwrap(); // 如果线程 panic 则 unwrap 也会 panic
    println!("Thread returned: {result}");
}
```

**Thread::spawn 的类型要求**：

```rust
// 闭包必须满足：
// 1. Send——可以转移到另一个线程
// 2. 'static——不能借用调用作用域中的数据
// 3. FnOnce——获取捕获变量的所有权

let data = vec![1, 2, 3];

// ❌ 借用了 data——不满足 'static
// thread::spawn(|| println!("{data:?}"));

// ✅ 将所有权转移到线程中
thread::spawn(move || println!("{data:?}"));
// data 在这里不再可访问
```

### 作用域线程（std::thread::scope）

从 Rust 1.63 开始，作用域线程解决了 `'static` 要求——线程可以借用父作用域中的数据：

```rust
use std::thread;

fn main() {
    let mut data = vec![1, 2, 3, 4, 5];

    thread::scope(|s| {
        // 线程 1：借用共享引用
        s.spawn(|| {
            let sum: i32 = data.iter().sum();
            println!("Sum: {sum}");
        });

        // 线程 2：也借用共享引用（多个读者可以）
        s.spawn(|| {
            let max = data.iter().max().unwrap();
            println!("Max: {max}");
        });

        // ❌ 当存在共享借用时不能可变借用：
        // s.spawn(|| data.push(6));
    });
    // 所有作用域线程在此处 join——保证在作用域返回之前完成

    // 现在可以安全修改了——所有线程都已结束
    data.push(6);
    println!("Updated: {data:?}");
}
```

> **这意义重大**：在作用域线程之前，你必须对所有东西 `Arc::clone()`
> 来与线程共享。现在你可以直接借用，编译器会证明所有线程在数据离开作用域之前都已结束。

### rayon —— 数据并行

`rayon` 提供并行迭代器，自动将工作分配到线程池上：

```rust,ignore
// Cargo.toml: rayon = "1"
use rayon::prelude::*;

fn main() {
    let data: Vec<u64> = (0..1_000_000).collect();

    // 顺序执行：
    let sum_seq: u64 = data.iter().map(|x| x * x).sum();

    // 并行——只需将 .iter() 改为 .par_iter()：
    let sum_par: u64 = data.par_iter().map(|x| x * x).sum();

    assert_eq!(sum_seq, sum_par);

    // 并行排序：
    let mut numbers = vec![5, 2, 8, 1, 9, 3];
    numbers.par_sort();

    // 用 map/filter/collect 进行并行处理：
    let results: Vec<_> = data
        .par_iter()
        .filter(|&&x| x % 2 == 0)
        .map(|&x| expensive_computation(x))
        .collect();
}

fn expensive_computation(x: u64) -> u64 {
    // 模拟 CPU 密集型计算
    (0..1000).fold(x, |acc, _| acc.wrapping_mul(7).wrapping_add(13))
}
```

**何时使用 rayon vs 线程**：

| 使用 | 何时 |
|-----|------|
| `rayon::par_iter()` | 并行处理集合（map、filter、reduce） |
| `thread::spawn` | 长时间运行的后台任务、I/O 工作线程 |
| `thread::scope` | 借用本地数据的短生命周期并行任务 |
| `async` + `tokio` | I/O 密集型并发（网络、文件 I/O） |

### 共享状态：Arc、Mutex、RwLock、原子类型

当线程需要共享可变状态时，Rust 提供了安全的抽象：

> **注意：** 在这些示例中，`.lock()`、`.read()` 和 `.write()` 上的 `.unwrap()` 是为了简洁。
> 这些调用只有在另一个线程持有锁时 panic（"锁中毒"，poisoning）才会失败。生产代码应决定是从中毒锁中恢复还是传播错误。

```rust
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

// --- Arc<Mutex<T>>：共享 + 独占访问 ---
fn mutex_example() {
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let mut guard = counter.lock().unwrap();
                *guard += 1;
            } // guard 被丢弃 → 锁释放
        }));
    }

    for h in handles { h.join().unwrap(); }
    println!("Counter: {}", counter.lock().unwrap()); // 10000
}

// --- Arc<RwLock<T>>：多个读者 或 一个写者 ---
fn rwlock_example() {
    let config = Arc::new(RwLock::new(String::from("initial")));

    // 多个读者——互不阻塞
    let readers: Vec<_> = (0..5).map(|id| {
        let config = Arc::clone(&config);
        thread::spawn(move || {
            let guard = config.read().unwrap();
            println!("Reader {id}: {guard}");
        })
    }).collect();

    // 写者——阻塞并等待所有读者完成
    {
        let mut guard = config.write().unwrap();
        *guard = "updated".to_string();
    }

    for r in readers { r.join().unwrap(); }
}

// --- 原子类型：用于简单值的无锁操作 ---
fn atomic_example() {
    let counter = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::Relaxed);
                // 无锁，无 mutex——硬件原子指令
            }
        }));
    }

    for h in handles { h.join().unwrap(); }
    println!("Atomic counter: {}", counter.load(Ordering::Relaxed)); // 10000
}
```

### 快速对比

| 原语 | 使用场景 | 成本 | 竞争 |
|-----------|----------|------|------------|
| `Mutex<T>` | 短临界区 | 加锁 + 解锁 | 线程排队等待 |
| `RwLock<T>` | 读多写少 | 读写锁 | 读者并发，写者独占 |
| `AtomicU64` 等 | 计数器、标志 | 硬件 CAS | 无锁——无需等待 |
| 通道 | 消息传递 | 队列操作 | 生产者/消费者解耦 |

### 条件变量（Condvar）

`Condvar` 让一个线程**等待**，直到另一个线程发出信号表示某个条件为真，而无需忙等待（busy-loop）。它总是与 `Mutex` 配对使用：

```rust
use std::sync::{Arc, Mutex, Condvar};
use std::thread;

let pair = Arc::new((Mutex::new(false), Condvar::new()));
let pair2 = Arc::clone(&pair);

// 生成的线程：等待直到 ready == true
let handle = thread::spawn(move || {
    let (lock, cvar) = &*pair2;
    let mut ready = lock.lock().unwrap();
    while !*ready {
        ready = cvar.wait(ready).unwrap(); // 原子地解锁 + 休眠
    }
    println!("Worker: condition met, proceeding");
});

// 主线程：设置 ready = true，然后发信号
{
    let (lock, cvar) = &*pair;
    let mut ready = lock.lock().unwrap();
    *ready = true;
    cvar.notify_one(); // 唤醒一个等待线程（多个用 notify_all）
}
handle.join().unwrap();
```

> **模式**：在 `wait()` 返回后，始终在 `while` 循环中重新检查条件——
> 操作系统允许虚假唤醒（spurious wakeups）。

### 延迟初始化：OnceLock 和 LazyLock

在 Rust 1.80 之前，初始化需要运行时计算（例如解析配置、编译正则表达式）的全局静态变量需要 `lazy_static!` 宏或 `once_cell` crate。标准库现在提供了两种原生类型来覆盖这些用例：

```rust
use std::sync::{OnceLock, LazyLock};
use std::collections::HashMap;

// OnceLock——通过 `get_or_init` 在首次使用时初始化。
// 当初始值依赖运行时参数时很有用。
static CONFIG: OnceLock<HashMap<String, String>> = OnceLock::new();

fn get_config() -> &'static HashMap<String, String> {
    CONFIG.get_or_init(|| {
        // 开销大：读取并解析配置文件——只发生一次。
        let mut m = HashMap::new();
        m.insert("log_level".into(), "info".into());
        m
    })
}

// LazyLock——首次访问时初始化，在定义处提供闭包。
// 等价于 lazy_static! 但无需宏。
static REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^[a-zA-Z0-9_]+$").unwrap()
});

fn is_valid_identifier(s: &str) -> bool {
    REGEX.is_match(s) // 首次调用编译正则；后续调用复用。
}
```

| 类型 | 稳定版本 | 初始化时机 | 适用场景 |
|------|-----------|-------------|----------|
| `OnceLock<T>` | Rust 1.70 | 调用处（`get_or_init`） | 初始化依赖运行时参数 |
| `LazyLock<T>` | Rust 1.80 | 定义处（闭包） | 初始化是自包含的 |
| `lazy_static!` | — | 定义处（宏） | 1.80 之前的代码库（建议迁移） |
| `const fn` + `static` | 一直可用 | 编译期 | 值可在编译期计算 |

> **迁移提示**：将 `lazy_static! { static ref X: T = expr; }` 替换为
> `static X: LazyLock<T> = LazyLock::new(|| expr);` —— 语义相同，无需宏，
> 无外部依赖。

### 无锁模式

对于高性能代码，可以完全避免锁：

```rust
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

// 模式 1：自旋锁（教学用途——优先使用 std::sync::Mutex）
// ⚠️ 警告：这仅是教学示例。真正的自旋锁需要：
//   - RAII guard（这样持有锁时 panic 不会永久死锁）
//   - 公平性保证（这在竞争下会饥饿）
//   - 退避策略（指数退避、让出 CPU 给 OS）
// 生产环境请使用 std::sync::Mutex 或 parking_lot::Mutex。
struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    fn new() -> Self { SpinLock { locked: AtomicBool::new(false) } }

    fn lock(&self) {
        while self.locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop(); // CPU 提示：我们正在自旋
        }
    }

    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

// 模式 2：无锁 SPSC（单生产者，单消费者）
// 生产环境使用 crossbeam::queue::ArrayQueue 或类似实现
// 自己手写仅供学习。

// 模式 3：用于无等待读取的序列计数器
// ⚠️ 最适合单个机器字大小的类型（u64、f64）；更宽的 T 读取时可能撕裂。
struct SeqLock<T: Copy> {
    seq: AtomicUsize,
    data: std::cell::UnsafeCell<T>,
}

unsafe impl<T: Copy + Send> Sync for SeqLock<T> {}

impl<T: Copy> SeqLock<T> {
    fn new(val: T) -> Self {
        SeqLock {
            seq: AtomicUsize::new(0),
            data: std::cell::UnsafeCell::new(val),
        }
    }

    fn read(&self) -> T {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 != 0 { continue; } // 写者正在进行，重试

            // SAFETY（安全说明）：我们使用 ptr::read_volatile 来防止编译器
            // 重排或缓存读取。SeqLock 协议（读取后检查 s1 == s2）
            // 确保如果有写者活动我们会重试。
            // 这镜像了 C 的 SeqLock 模式，其中数据读取必须使用
            // volatile/relaxed 语义以避免并发下的撕裂。
            let value = unsafe { core::ptr::read_volatile(self.data.get() as *const T) };

            // Acquire 屏障：确保上面的数据读取在
            // 我们重新检查序列计数器之前被排序。
            std::sync::atomic::fence(Ordering::Acquire);
            let s2 = self.seq.load(Ordering::Relaxed);

            if s1 == s2 { return value; } // 没有写者介入
            // 否则重试
        }
    }

    /// # 安全契约
    /// 同一时间只有一个线程可以调用 `write()`。如果需要多个写者，
    /// 请将 `write()` 调用包装在外部 `Mutex` 中。
    fn write(&self, val: T) {
        // 递增为奇数（表示写入正在进行）。
        // AcqRel：Acquire 侧防止后续的数据写入
        // 被重排到此递增之前（读者必须先看到奇数才能观察到部分写入）。
        // Release 侧对于单写者技术上来说不是必需的，
        // 但无害且保持一致。
        self.seq.fetch_add(1, Ordering::AcqRel);
        // SAFETY（安全说明）：单写者不变量由调用者维护（见上面的文档）。
        // UnsafeCell 允许内部可变性；序列计数器保护读者。
        unsafe { *self.data.get() = val; }
        // 递增为偶数（表示写入完成）。
        // Release：确保数据写入在读者看到偶数序列之前可见。
        self.seq.fetch_add(1, Ordering::Release);
    }
}
```

> **⚠️ Rust 内存模型警告**：`write()` 中通过 `UnsafeCell` 的非原子写入
> 与 `read()` 中的非原子 `ptr::read_volatile` 并发，在 Rust 抽象机下技术上是数据竞争——
> 即使 SeqLock 协议确保读者总是会在过时数据上重试。这镜像了
> C 内核的 SeqLock 模式，在实践中对于能放入单个机器字（例如 `u64`）的类型 `T`
> 在所有现代硬件上都是可靠的。对于更宽的类型，
> 考虑为数据字段使用 `AtomicU64` 或将访问包装在 `Mutex` 中。
> 参见 [Rust unsafe 代码指南](https://rust-lang.github.io/unsafe-code-guidelines/)
> 了解关于 `UnsafeCell` 并发的演进情况。

> **实用建议**：无锁代码很难写对。除非性能分析显示锁竞争是你的瓶颈，否则使用 `Mutex` 或
> `RwLock`。当你确实需要无锁时，优先使用经过验证的 crate（`crossbeam`、`arc-swap`、`dashmap`），
> 而非自己手写。

> **核心要点 —— 并发**
> - 作用域线程（`thread::scope`）让你无需 `Arc` 即可借用栈数据
> - `rayon::par_iter()` 用一个方法调用就能并行化迭代器
> - 使用 `OnceLock`/`LazyLock` 替代 `lazy_static!`；在求助于原子类型之前先使用 `Mutex`
> - 无锁代码很难写对——优先使用经过验证的 crate，而非手写实现

> **参见：** [第 5 章 —— 通道](ch05-channels-and-message-passing.md) 了解消息传递并发。[第 8 章 —— 智能指针](ch09-smart-pointers-and-interior-mutability.md) 了解 Arc/Rc 的细节。

```mermaid
flowchart TD
    A["需要共享<br>可变状态？"] -->|是| B{"竞争<br>程度多大？"}
    A -->|否| C["使用通道<br>（第 5 章）"]

    B -->|"读多写少"| D["RwLock"]
    B -->|"短临界区"| E["Mutex"]
    B -->|"简单计数器<br>或标志"| F["原子类型（Atomics）"]
    B -->|"复杂状态"| G["Actor + 通道"]

    H["需要并行？"] -->|"集合<br>处理"| I["rayon::par_iter"]
    H -->|"后台任务"| J["thread::spawn"]
    H -->|"借用本地数据"| K["thread::scope"]

    style A fill:#e8f4f8,stroke:#2980b9,color:#000
    style B fill:#fef9e7,stroke:#f1c40f,color:#000
    style C fill:#d4efdf,stroke:#27ae60,color:#000
    style D fill:#fdebd0,stroke:#e67e22,color:#000
    style E fill:#fdebd0,stroke:#e67e22,color:#000
    style F fill:#fdebd0,stroke:#e67e22,color:#000
    style G fill:#fdebd0,stroke:#e67e22,color:#000
    style H fill:#e8f4f8,stroke:#2980b9,color:#000
    style I fill:#d4efdf,stroke:#27ae60,color:#000
    style J fill:#d4efdf,stroke:#27ae60,color:#000
    style K fill:#d4efdf,stroke:#27ae60,color:#000
```

---

### 练习：使用作用域线程的并行 Map ★★（约 25 分钟）

编写一个函数 `parallel_map<T, R>(data: &[T], f: fn(&T) -> R, num_threads: usize) -> Vec<R>`，将 `data` 拆分为 `num_threads` 个块，每个块在一个作用域线程中处理。不要使用 `rayon`——使用 `std::thread::scope`。

<details>
<summary>🔑 答案</summary>

```rust
fn parallel_map<T: Sync, R: Send>(data: &[T], f: fn(&T) -> R, num_threads: usize) -> Vec<R> {
    let chunk_size = (data.len() + num_threads - 1) / num_threads;
    let mut results = Vec::with_capacity(data.len());

    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in data.chunks(chunk_size) {
            handles.push(s.spawn(move || {
                chunk.iter().map(f).collect::<Vec<_>>()
            }));
        }
        for h in handles {
            results.extend(h.join().unwrap());
        }
    });

    results
}

fn main() {
    let data: Vec<u64> = (1..=20).collect();
    let squares = parallel_map(&data, |x| x * x, 4);
    assert_eq!(squares, (1..=20).map(|x: u64| x * x).collect::<Vec<_>>());
    println!("Parallel squares: {squares:?}");
}
```

</details>

***
