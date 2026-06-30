# 毕业项目：类型安全的任务调度器

本项目将全书各章的模式整合到一个生产风格的系统中。你将构建一个**类型安全的、并发的任务调度器**，它使用泛型、trait、类型状态（typestate）、通道、错误处理和测试。

**预计时间**：4–6 小时 | **难度**：★★★

> **你将练习：**
> - 泛型与 trait 约束（第 1–2 章）
> - 用于任务生命周期的类型状态模式（第 3 章）
> - 用于零开销状态标记的 PhantomData（第 4 章）
> - 用于工作线程通信的通道（第 5 章）
> - 使用作用域线程的并发（第 6 章）
> - 使用 `thiserror` 的错误处理（第 9 章）
> - 基于属性的测试（第 13 章）
> - 使用 `TryFrom` 和已验证类型的 API 设计（第 14 章）

## 问题

构建一个任务调度器，其中：

1. **任务**有类型化的生命周期：`Pending → Running → Completed`（或 `Failed`）
2. **工作线程**从通道拉取任务，执行它们，并报告结果
3. **调度器**管理任务提交、工作线程协调和结果收集
4. 非法状态转换是**编译期错误**

```mermaid
stateDiagram-v2
    [*] --> Pending: scheduler.submit(task)
    Pending --> Running: 工作线程取出任务
    Running --> Completed: 任务成功
    Running --> Failed: 任务返回 Err
    Completed --> [*]: scheduler.results()
    Failed --> [*]: scheduler.results()

    Pending --> Pending: ❌ 不能直接执行
    Completed --> Running: ❌ 不能重新运行
```

## 第 1 步：定义任务类型

从类型状态标记和一个泛型 `Task` 开始：

```rust
// → std::marker::PhantomData<T>：零大小标记，携带类型层面的关系信息。
use std::marker::PhantomData;

// --- 状态标记（零大小） ---
// → 单元结构体作为 typestate 标签，运行时无数据，仅用于编译期类型区分。
struct Pending;
struct Running;
struct Completed;
struct Failed;

// --- 任务 ID（用于类型安全的新类型） ---
// → TaskId(u64) 新类型：防止与其他 u64 混淆，并派生常用 trait。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TaskId(u64);

// --- Task 结构体，按生命周期状态参数化 ---
// → <State, R>：State 是 typestate（Pending/Running/...），R 是任务返回类型。
//   PhantomData 携带这些类型信息而不存储实际值，运行时零开销。
struct Task<State, R> {
    id: TaskId,
    name: String,
    _state: PhantomData<State>,
    _result: PhantomData<R>,
}
```

**你的任务**：实现状态转换，使得：
- `Task<Pending, R>` 可以转换为 `Task<Running, R>`（通过 `start()`）
- `Task<Running, R>` 可以转换为 `Task<Completed, R>` 或 `Task<Failed, R>`
- 其他任何转换都无法编译

<details>
<summary>💡 提示</summary>

每个转换方法应消耗 `self` 并返回新状态：

```rust
// → impl<R> Task<Pending, R>：只为 Pending 状态实现 start。
//   编译器据此保证只有 Pending 任务能调用 start —— 非法转换无法编译。
impl<R> Task<Pending, R> {
    // → start(self) -> Task<Running, R>：消耗 self（所有权转移），
    //   返回新状态的任务。R 是泛型（任务返回类型任意）。
    fn start(self) -> Task<Running, R> {
        Task {
            id: self.id,
            name: self.name,
            _state: PhantomData,
            _result: PhantomData,
        }
    }
}
```

</details>

## 第 2 步：定义工作函数

任务需要一个要执行的函数。使用装箱的闭包：

```rust
// → WorkItem<R>：携带可执行工作的任务项。
//   R: Send + 'static 约束：
//   - Send：R 可跨线程移动（结果需从工作线程传回）。
//   - 'static：R 不含非 'static 引用（任务可存活任意长）。
struct WorkItem<R: Send + 'static> {
    id: TaskId,
    name: String,
    // → Box<dyn FnOnce() -> Result<R, String> + Send>：
    //   装箱的、类型擦除的闭包。
    //   - FnOnce：仅调用一次（消耗捕获环境）。
    //   - dyn：trait 对象，运行时分发，支持不同闭包类型。
    //   - + Send：闭包可跨线程移动（被 spawn 到工作线程）。
    work: Box<dyn FnOnce() -> Result<R, String> + Send>,
}
```

**你的任务**：实现 `WorkItem::new()`，它接受任务名和闭包。
添加一个 `TaskId` 生成器（简单的原子计数器或受 mutex 保护的计数器）。

## 第 3 步：错误处理

使用 `thiserror` 定义调度器的错误类型：

```rust,ignore
// → thiserror::Error：派生宏，自动实现 std::error::Error + Display。
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchedulerError {
    // → #[error("...")]：指定 Display 文本。
    #[error("scheduler is shut down")]
    ShutDown,

    // → 元组变体：{0:?} 用 Debug 格式化第 0 个字段（TaskId 需 Debug）。
    #[error("task {0:?} failed: {1}")]
    TaskFailed(TaskId, String),

    #[error("channel send error")]
    // → #[from]：自动实现 From<SendError<()>> for SchedulerError，
    //   使 ? 能将发送错误自动转换为此变体。
    ChannelError(#[from] std::sync::mpsc::SendError<()>),

    #[error("worker panicked")]
    WorkerPanic,
}
```

## 第 4 步：调度器

使用通道（第 5 章）和作用域线程（第 6 章）构建调度器：

```rust
// → std::sync::mpsc：同步多生产者单消费者通道，用于线程间通信。
use std::sync::mpsc;

struct Scheduler<R: Send + 'static> {
    // → Option<Sender>：用 Option 以便 shutdown 时 take() 取出并 drop，
    //   关闭任务通道触发工作线程退出。
    sender: Option<mpsc::Sender<WorkItem<R>>>,
    // → Receiver<TaskResult<R>>：收集工作线程返回的结果。
    results: mpsc::Receiver<TaskResult<R>>,
    num_workers: usize,
}

struct TaskResult<R> {
    id: TaskId,
    name: String,
    // → outcome 携带任务执行结果：Ok(成功值) 或 Err(错误消息)。
    //   把失败包装进结果而非 panic，保证调度器整体健壮。
    outcome: Result<R, String>,
}
```

**你的任务**：实现：
- `Scheduler::new(num_workers: usize) -> Self` — 创建通道并派生工作线程
- `Scheduler::submit(&self, item: WorkItem<R>) -> Result<TaskId, SchedulerError>`
- `Scheduler::shutdown(self) -> Vec<TaskResult<R>>` — 丢弃发送端，join 工作线程，收集结果

<details>
<summary>💡 提示 — 工作线程循环</summary>

```rust
// → worker_loop 是工作线程的主循环：拉取任务、执行、回传结果。
//   泛型 <R: Send + 'static>：任务返回类型需跨线程。
fn worker_loop<R: Send + 'static>(
    // → Arc<Mutex<Receiver>>：多工作线程共享单一接收端。
    //   Arc 提供共享所有权，Mutex 互斥访问（防止并发 recv 数据竞争）。
    rx: std::sync::Arc<std::sync::Mutex<mpsc::Receiver<WorkItem<R>>>>,
    // → result_tx：每个工作线程持有一个结果发送端副本。
    result_tx: mpsc::Sender<TaskResult<R>>,
    worker_id: usize,
) {
    loop {
        // → 块作用域限制锁：lock 持有期间 recv 阻塞会卡住其他线程，
        //   故只在拿值瞬间持锁。recv 在锁内完成，解锁后处理。
        let item = {
            let rx = rx.lock().unwrap();
            rx.recv()
        };
        match item {
            Ok(work_item) => {
                // → (work_item.work)()：调用装箱闭包执行实际工作。
                //   FnOnce 仅可调用一次，此处消耗闭包。
                let outcome = (work_item.work)();
                // → result_tx.send：回传结果。let _ = 忽略发送错误
                //   （收集端可能已关闭，此时无需 panic）。
                let _ = result_tx.send(TaskResult {
                    id: work_item.id,
                    name: work_item.name,
                    outcome,
                });
            }
            Err(_) => break, // 通道关闭
        }
    }
}
```

</details>

## 第 5 步：集成测试

编写测试来验证：

1. **正常路径**：提交 10 个任务，关闭，验证 10 个结果都是 `Ok`
2. **错误处理**：提交会失败的任务，验证 `TaskResult.outcome` 是 `Err`
3. **空调度器**：创建后立即关闭 — 无 panic
4. **属性测试**（附加）：使用 `proptest` 验证对于任意 N 个任务（1..100），调度器总是返回恰好 N 个结果

```rust
// → #[cfg(test)]：测试模块仅在 cargo test 时编译。
#[cfg(test)]
mod tests {
    // → use super::*：导入父模块（被测模块）的所有项。
    use super::*;

    #[test]
    fn happy_path() {
        // → Scheduler::<String>::new(4)：turbofish 指定任务返回类型为 String，
        //   创建 4 个工作线程的调度器。
        let scheduler = Scheduler::<String>::new(4);

        for i in 0..10 {
            // → WorkItem::new：构造任务项。move 闭包捕获 i。
            let item = WorkItem::new(
                format!("task-{i}"),
                move || Ok(format!("result-{i}")),
            );
            scheduler.submit(item).unwrap();
        }

        // → shutdown：关闭调度器并收集所有结果。
        let results = scheduler.shutdown();
        assert_eq!(results.len(), 10);
        // → 遍历断言每个任务都成功。
        for r in &results {
            assert!(r.outcome.is_ok());
        }
    }

    #[test]
    fn handles_failures() {
        let scheduler = Scheduler::<String>::new(2);

        // → 一个返回 Ok、一个返回 Err 的任务，验证错误被捕获而非 panic。
        scheduler.submit(WorkItem::new("good", || Ok("ok".into()))).unwrap();
        scheduler.submit(WorkItem::new("bad", || Err("boom".into()))).unwrap();

        let results = scheduler.shutdown();
        assert_eq!(results.len(), 2);

        // → results.iter().filter(...).collect()：用迭代器筛选失败结果。
        let failures: Vec<_> = results.iter()
            .filter(|r| r.outcome.is_err())
            .collect();
        assert_eq!(failures.len(), 1);
    }
}
```

## 第 6 步：整合在一起

这是展示完整系统的 `main()`：

```rust,ignore
fn main() {
    let scheduler = Scheduler::<String>::new(4);

    // 提交不同工作量的任务
    for i in 0..20 {
        let item = WorkItem::new(
            format!("compute-{i}"),
            move || {
                // 模拟工作
                // → std::thread::sleep：同步阻塞线程（此处模拟 CPU 工作）。
                std::thread::sleep(std::time::Duration::from_millis(10));
                // → 每 7 个任务人为制造一次失败，演示错误处理。
                if i % 7 == 0 {
                    Err(format!("task {i} hit a simulated error"))
                } else {
                    Ok(format!("task {i} completed with value {}", i * i))
                }
            },
        );
        // 注意：为简洁起见使用 .unwrap() — 生产环境中应处理 SendError。
        scheduler.submit(item).unwrap();
    }

    println!("All tasks submitted. Shutting down...");
    // → shutdown 阻塞直到所有任务完成，返回结果向量。
    let results = scheduler.shutdown();

    // → Iterator::partition：按闭包把迭代器分为两组（满足/不满足），
    //   返回两个 Vec 组成的元组。这里按成功/失败划分。
    let (ok, err): (Vec<_>, Vec<_>) = results.iter()
        .partition(|r| r.outcome.is_ok());

    println!("\n✅ Succeeded: {}", ok.len());
    for r in &ok {
        // → Result::as_ref()：将 &Result<T,E> 转为 Result<&T,&E>，
        //   unwrap() 取出 &T 引用用于打印（不消耗原值）。
        println!("  {} → {}", r.name, r.outcome.as_ref().unwrap());
    }

    println!("\n❌ Failed: {}", err.len());
    for r in &err {
        // → unwrap_err：取出错误引用（断言是 Err）。
        println!("  {} → {}", r.name, r.outcome.as_ref().unwrap_err());
    }
}
```

## 评估标准

| 标准 | 目标 |
|-----------|--------|
| 类型安全 | 非法状态转换无法编译 |
| 并发 | 工作线程并行运行，无数据竞争 |
| 错误处理 | 所有失败都捕获在 `TaskResult` 中，无 panic |
| 测试 | 至少 3 个测试；使用 proptest 为加分项 |
| 代码组织 | 清晰的模块结构，公共 API 使用已验证类型 |
| 文档 | 关键类型有解释不变式的文档注释 |

## 扩展想法

基础调度器工作后，尝试这些增强：

1. **优先级队列**：添加一个 `Priority` 新类型（1–10），优先处理高优先级任务
2. **重试策略**：失败任务在被标记为永久失败前最多重试 N 次
3. **取消**：添加 `cancel(TaskId)` 方法来移除待处理任务
4. **Async 版本**：移植到 `tokio::spawn` 和 `tokio::sync::mpsc` 通道（第 15 章）
5. **指标**：追踪每个工作线程的任务数、平均执行时间和失败率

***
