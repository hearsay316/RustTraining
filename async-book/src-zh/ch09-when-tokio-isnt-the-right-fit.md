# 9. 当 Tokio 不合适时

> **你将学到什么：**
> - `'static` 困境：当 `tokio::spawn` 迫使你四处使用 `Arc` 时
> - `LocalSet`：为 `!Send` Future 设计的单线程执行环境
> - `FuturesUnordered`：借用友好的并发（无需 spawn）
> - `JoinSet`：托管的 spawn 任务集合
> - 编写与运行时（runtime）无关的库

```mermaid
graph TD
    START["需要并发 Future？"] --> STATIC{"Future 是否满足 'static？"}
    STATIC -->|是| SEND{"Future 是否为 Send？"}
    STATIC -->|否| FU["FuturesUnordered<br/>在当前 Task 上运行"]
    SEND -->|是| SPAWN["tokio::spawn<br/>多线程"]
    SEND -->|否| LOCAL["LocalSet<br/>单线程"]
    SPAWN --> MANAGE{"需要跟踪/中止 Task？"}
    MANAGE -->|是| JOINSET["JoinSet / TaskTracker"]
    MANAGE -->|否| HANDLE["JoinHandle"]

    style START fill:#f5f5f5,stroke:#333,color:#000
    style FU fill:#d4efdf,stroke:#27ae60,color:#000
    style SPAWN fill:#e8f4f8,stroke:#2980b9,color:#000
    style LOCAL fill:#fef9e7,stroke:#f39c12,color:#000
    style JOINSET fill:#e8daef,stroke:#8e44ad,color:#000
    style HANDLE fill:#e8f4f8,stroke:#2980b9,color:#000
```

## 'static Future 困境

Tokio 的 `spawn` 要求 `'static` Future。这意味着你无法在 spawn 的任务中借用局部数据：

```rust
// ============================================================
// 核心概念：'static 约束的根源
// ============================================================
// tokio::spawn 将 Future 所有权移交给运行时，运行时线程池可以在
// 调用者函数返回后继续执行该任务。因此任何借用（&T）都无法被
// 编译器证明在任务执行期间始终有效——必须全部使用 owned 数据。
// 设计理由：这是 Rust 所有权模型对安全并发的自然约束，
// 避免了 Go 中 goroutine 引用悬垂数据的运行时风险。
// ============================================================

async fn process_items(items: &[String]) {
    // ❌ 不能这样做 —— items 是借用的引用，不满足 'static
    // for item in items {
    //     tokio::spawn(async {
    //         process(item).await;
    //     });
    // }

    // 😐 妥协方案 1：逐个克隆
    for item in items {
        let item = item.clone(); // → 复制数据，获得 owned String
        tokio::spawn(async move {
            process(&item).await; // item 已被移入，满足 'static
        });
    }

    // 😐 妥协方案 2：Arc + 共享引用
    let items = Arc::new(items.to_vec()); // 所有数据放入 Arc
    for i in 0..items.len() {
        let items = Arc::clone(&items);    // 每个任务持有一个 Arc 句柄
        tokio::spawn(async move {
            process(&items[i]).await;      // 通过 Arc 索引访问，没有借用
        });
    }
}
```

这确实烦人。在 Go 中，你只需要写 `go func() { use(item) }()`。但在 Rust 中，所有权制度迫使你思考谁拥有数据以及它的生命周期有多长——这是编译期安全保障的代价。

### `tokio::spawn` 的替代方案

并非每个并发问题都需要 `spawn`。以下是三个工具，各自解决一种*不同的*约束：

```rust
// ============================================================
// 核心概念：三种 spawn 替代方案及其适用场景
// ============================================================
// 1. FuturesUnordered → 解决 'static 约束
//    在当前任务本地驱动多个 Future，不涉及线程迁移
// 2. LocalSet → 解决 Send 约束（但仍需 'static）
//    在单线程上运行 !Send 类型（Rc、Cell、RefCell 等）
// 3. JoinSet → 解决任务管理（仍需 'static + Send）
//    提供自动清理、批量取消、结果收集
// ============================================================

// --- 1. FuturesUnordered：完全避开 'static 限制 ---
// 核心原理：所有 Future 都在当前任务上被 poll，没有线程迁移，
// 因此不需要 Send + 'static。共享一个任务的好处是零内存分配开销，
// 代价是所有 Future 串行 poll——一个阻塞会影响其他。
use futures::stream::{FuturesUnordered, StreamExt};

async fn process_items_with_futures(items: &[String]) {
    let futures: FuturesUnordered<_> = items
        .iter()
        .map(|item| async move {
            // ✅ 直接在闭包中借用 item，不需要 clone
            //    因为 async 块在当前上下文被 poll，
            //    item 的引用在 await 点之前始终有效
            process(item).await
        })
        .collect();
    // ↑ .collect() 将所有 Future 收集到一个集合中

    // for_each 逐一等待每个 Future 完成
    futures.for_each(|result| async move {
        println!("Result: {result:?}");
    }).await;
    // → 所有 Future 执行完毕
}

// --- 2. tokio::task::LocalSet：在当前线程运行 !Send Future ---
// ⚠️ 注意：仍然需要 'static，只是解除了 Send 约束
// 适用场景：需要 Rc/Cell/RefCell 等 !Send 类型，但愿意放弃多线程
use tokio::task::LocalSet;

let local_set = LocalSet::new();
local_set.run_until(async {
    tokio::task::spawn_local(async {
        // → 这里可以使用 Rc、Cell 等 !Send 类型
        let rc = std::rc::Rc::new(42);
        println!("{rc}");
    }).await.unwrap();
}).await;
// → run_until 在当前线程上驱动所有 spawn_local 任务

// --- 3. tokio::task::JoinSet (tokio 1.21+)：托管的任务组 ---
// ⚠️ 注意：仍然需要 'static + Send
// 解决的是任务*管理*问题，而非 'static 或 Send 问题
// 适用场景：动态创建任务，需要跟踪、批量取消、按完成顺序获取结果
use tokio::task::JoinSet;

async fn with_joinset() {
    let mut set = JoinSet::new();

    for i in 0..10 {
        // i 是 i32（Copy 类型），自动被复制移入闭包
        // 对于借用数据，仍需 Arc 或 clone
        set.spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            i * 2  // → 任务返回值
        });
    }

    // join_next() 按完成顺序返回结果（不是 spawn 顺序）
    while let Some(result) = set.join_next().await {
        println!("Task completed: {:?}", result.unwrap());
    }
    // → JoinSet 在 drop 时会自动 abort 所有未完成的任务
}
```

> **哪个工具解决哪个问题？**
>
> | 你遇到的约束 | 工具 | 避开 `'static`？ | 避开 `Send`？ |
> |---|---|---|---|
> | 无法让 Future 满足 `'static` | `FuturesUnordered` | ✅ 是 | ✅ 是 |
> | Future 是 `'static` 但是 `!Send` | `LocalSet` | ❌ 否 | ✅ 是 |
> | 需要跟踪/中止 spawned 任务 | `JoinSet` | ❌ 否 | ❌ 否 |

### 为库编写与运行时无关的代码

如果你正在编写一个库——不要强制用户绑定特定运行时：

```rust
// ============================================================
// 核心概念：与运行时无关的库设计
// ============================================================
// 库代码应只依赖 std::future::Future 和 futures crate，
// 避免直接依赖 tokio（或其他运行时）。
// 设计理由：让库用户自由选择 tokio/smol/async-std 等运行时，
// 保持生态系统的可组合性。应用程序层面才绑定具体运行时。
// ============================================================

// ❌ 糟糕：库强加 tokio 依赖
pub async fn my_lib_function() {
    tokio::time::sleep(Duration::from_secs(1)).await;
    // → 调用这个函数的用户也被迫引入 tokio
}

// ✅ 良好：库只依赖标准库 + futures
pub async fn my_lib_function() {
    // 纯计算逻辑，不依赖任何运行时设施
    do_computation().await;
}

// ✅ 良好：通过泛型接受 I/O 操作
// 调用者传入具体的 Future 工厂
pub async fn fetch_with_retry<F, Fut, T, E>(
    operation: F,
    max_retries: usize,
) -> Result<T, E>
where
    F: Fn() -> Fut,                    // 工厂闭包
    Fut: Future<Output = Result<T, E>>, // 返回的 Future
{
    for attempt in 0..max_retries {
        match operation().await {       // → 每次重试调用工厂新建 Future
            Ok(val) => return Ok(val),
            Err(e) if attempt == max_retries - 1 => return Err(e),
            // ↑ 最后一次尝试也失败时才返回错误
            Err(_) => continue,          // 中间失败继续重试
        }
    }
    unreachable!()
}
```

> **经验法则**：库应该依赖 `futures` crate，而不是 `tokio`。
> 应用程序应该依赖 `tokio`（或它们选择的其他运行时）。
> 这样能保持整个生态系统的可组合性。

<details>
<summary><strong>练习：FuturesUnordered vs Spawn</strong>（点击展开）</summary>

**挑战**：以两种方式编写同一个函数——用 `tokio::spawn`（需要 `'static`）和用 `FuturesUnordered`（借用数据）。该函数接收 `&[String]`，在模拟异步（async）查找后返回每个字符串的长度。

比较：哪种方式需要 `.clone()`？哪种可以直接借用输入切片？

<details>
<summary>参考答案</summary>

```rust
// ============================================================
// 对比：spawn（需要 clone） vs FuturesUnordered（零拷贝借用）
// ============================================================
// spawn 方案：每个任务独立运行在不同线程上
//   → 需要 clone 数据以满足 'static（多线程安全性）
// FuturesUnordered 方案：所有 Future 在当前任务内被 poll
//   → 直接借用 &[String] 即可（无跨线程迁移）
//   → 结果按完成顺序返回，非 spawn 顺序
// ============================================================

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::time::{sleep, Duration};

// 版本 1：tokio::spawn —— 需要 'static，必须克隆
async fn lengths_with_spawn(items: &[String]) -> Vec<usize> {
    let mut handles = Vec::new();
    for item in items {
        let owned = item.clone(); // 必须克隆 —— spawn 需要 'static
        handles.push(tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            owned.len() // → 使用 owned 数据
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
        // → 按 spawn 顺序收集结果（串行 join）
    }
    results
}

// 版本 2：FuturesUnordered —— 借用数据，零克隆
async fn lengths_without_spawn(items: &[String]) -> Vec<usize> {
    let futures: FuturesUnordered<_> = items
        .iter()
        .map(|item| async move {
            sleep(Duration::from_millis(10)).await;
            item.len() // ✅ 直接借用 item —— 不需要 clone
        })
        .collect();

    futures.collect().await
    // → collect() 按完成顺序收集结果（最快完成的先返回）
}

#[tokio::test]
async fn test_both_versions() {
    let items = vec!["hello".into(), "world".into(), "rust".into()];

    let v1 = lengths_with_spawn(&items).await;
    // v1 保持插入顺序（逐个 await）

    let mut v2 = lengths_without_spawn(&items).await;
    v2.sort(); // FuturesUnordered 按完成顺序返回，需要排序对比

    assert_eq!(v1, vec![5, 5, 4]);
    assert_eq!(v2, vec![4, 5, 5]);
}
```

**关键要点**：`FuturesUnordered` 通过在当前任务上运行所有 future 来避开 `'static` 要求（不涉及线程迁移）。权衡：所有 future 共享一个任务——如果某个 future 执行了长时间 CPU 计算，会阻塞其他 future 被 poll。CPU 密集型工作应使用 `spawn` 放到独立线程上。

</details>
</details>

> **关键要点 -- 当 Tokio 不合适时**
> - `FuturesUnordered` 在当前任务上并发运行多个 Future——无 `'static` 要求
> - `LocalSet` 在单线程执行器（executor）上启用 `!Send` future
> - `JoinSet` (tokio 1.21+) 为托管的 spawn 任务组提供自动清理和按完成顺序取结果
> - 对于库：只依赖 `std::future::Future` + `futures` crate，不直接依赖 tokio

> **另请参阅：** [第 8 章 -- Tokio 深入探究](ch08-tokio-deep-dive.md) 了解何时 spawn 是正确的工具，[第 11 章 -- Stream](ch11-streams-and-asynciterator.md) 了解 `buffer_unordered()` 作为另一个并发限制工具

***

