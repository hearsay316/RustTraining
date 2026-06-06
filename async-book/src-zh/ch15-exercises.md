## 练习

### 练习 1：异步 Echo 服务器

构建一个同时处理多个客户端的 TCP 回显服务器。

**要求**：
- 收听`127.0.0.1:8080`
- 接受连接并回显每行
- 优雅地处理客户端断开连接
- 当客户端连接/断开时打印日志

<details>
<summary>🔑 参考答案</summary>

```rust
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Echo server listening on :8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("[{addr}] Connected");

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        println!("[{addr}] Disconnected");
                        break;
                    }
                    Ok(_) => {
                        print!("[{addr}] Echo: {line}");
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

### 练习 2：具有速率限制的并发 URL 提取器

并发获取 URL 列表，最多 5 个并发请求。

<details>
<summary>🔑 参考答案</summary>

```rust
use futures::stream::{self, StreamExt};
use tokio::time::{sleep, Duration};

async fn fetch_urls(urls: Vec<String>) -> Vec<Result<String, String>> {
    // buffer_unordered(5) 确保最多轮询 5 个 Future
    // 同时——这里不需要单独的Semaphore。
    let results: Vec<_> = stream::iter(urls)
        .map(|url| {
            async move {
                println!("Fetching: {url}");

                match reqwest::get(&url).await {
                    Ok(resp) => match resp.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("{url}: {e}")),
                    },
                    Err(e) => Err(format!("{url}: {e}")),
                }
            }
        })
        .buffer_unordered(5) // ← 仅此一项就将并发限制为 5
        .collect()
        .await;

    results
}

// 注意：当您需要限制并发时，请使用Semaphore
// 独立生成的任务 (tokio::spawn)。使用 buffer_unordered
// 处理流时。不要将两者组合以获得相同的限制。
```

</details>

---

### 练习 3：使用工作池正常关闭

使用以下命令构建任务处理器：
- 基于通道的工作队列
- N 个工作任务从队列中消耗
- 按 Ctrl+C 优雅关闭：停止接受，完成正在进行的工作

<details>
<summary>🔑 参考答案</summary>

```rust
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};

struct WorkItem {
    id: u64,
    payload: String,
}

#[tokio::main]
async fn main() {
    let (work_tx, work_rx) = mpsc::channel::<WorkItem>(100);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // 生成 4 个工人
    let mut worker_handles = Vec::new();
    let work_rx = std::sync::Arc::new(tokio::sync::Mutex::new(work_rx));

    for id in 0..4 {
        let rx = work_rx.clone();
        let mut shutdown = shutdown_rx.clone();
        let handle = tokio::spawn(async move {
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
                        println!("Worker {id}: processing item {}", work.id);
                        sleep(Duration::from_millis(200)).await; // 模拟工作
                        println!("Worker {id}: done with item {}", work.id);
                    }
                    None => {
                        println!("Worker {id}: channel closed, exiting");
                        break;
                    }
                }
            }
        });
        worker_handles.push(handle);
    }

    // 生产者：提交一些工作
    let producer = tokio::spawn(async move {
        for i in 0..20 {
            let _ = work_tx.send(WorkItem {
                id: i,
                payload: format!("task-{i}"),
            }).await;
            sleep(Duration::from_millis(50)).await;
        }
    });

    // 等待 Ctrl+C
    tokio::signal::ctrl_c().await.unwrap();
    println!("\nShutdown signal received!");
    shutdown_tx.send(true).unwrap();
    producer.abort(); // 取消生产者任务

    // 等待工人完成
    for handle in worker_handles {
        let _ = handle.await;
    }
    println!("All workers shut down. Goodbye!");
}
```

</details>

---

### 练习 4：从头开始构建一个简单的异步 Mutex

使用通道实现异步感知互斥体（不使用`tokio::sync::Mutex`）。

*提示*：使用具有 1 个许可的 `tokio::sync::Semaphore` 来序列化访问。

<details>
<summary>🔑 参考答案</summary>

```rust
use std::cell::UnsafeCell;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct SimpleAsyncMutex<T> {
    data: Arc<UnsafeCell<T>>,
    semaphore: Arc<Semaphore>,
}

// SAFETY：对 T 的访问由信号量序列化（最多 1 个许可）。
unsafe impl<T: Send> Send for SimpleAsyncMutex<T> {}
unsafe impl<T: Send> Sync for SimpleAsyncMutex<T> {}

pub struct SimpleGuard<T> {
    data: Arc<UnsafeCell<T>>,
    _permit: OwnedSemaphorePermit, // guard 被丢弃时释放锁
}

impl<T> SimpleAsyncMutex<T> {
    pub fn new(value: T) -> Self {
        SimpleAsyncMutex {
            data: Arc::new(UnsafeCell::new(value)),
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    pub async fn lock(&self) -> SimpleGuard<T> {
        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        SimpleGuard {
            data: self.data.clone(),
            _permit: permit,
        }
    }
}

impl<T> std::ops::Deref for SimpleGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY：我们拥有唯一的信号灯许可，因此没有其他信号灯许可
        // SimpleGuard 存在，因此保证独占访问。
        unsafe { &*self.data.get() }
    }
}

impl<T> std::ops::DerefMut for SimpleGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY：同样的道理——单一许可保证排他性。
        unsafe { &mut *self.data.get() }
    }
}

// 当 SimpleGuard 被删除时，_permit 也被删除，
// 它会释放信号量许可，另一个 lock() 可以继续。

// 用法：
// let mutex = SimpleAsyncMutex::new(vec![1, 2, 3]);
// {
//     let mut guard = mutex.lock().await;
//     guard.push(4);
// } // 许可在这里释放
```

**关键要点**：异步互斥体通常构建在信号量之上。信号量提供异步等待机制 - 当锁定时，`acquire()` 挂起任务，直到许可被释放。这正是 `tokio::sync::Mutex` 的内部工作原理。

> **为什么是 `UnsafeCell` 而不是 `std::sync::Mutex`？** 此版本的先前版本
> 练习使用 `Arc<Mutex<T>>` 和 `Deref`/`DerefMut` 调用 `.lock().unwrap()`。
> 这无法编译 - 返回的 `&T` 借用了临时的 `MutexGuard`
> 立即被删除。 `UnsafeCell` 避开了中间的守卫，并且
> 基于信号量的序列化使 `unsafe` 发出声音。

</details>

---

### 练习 5：Stream 管道

使用流构建数据处理管道：
1. 生成数字 1..=100
2. 过滤到偶数
3. 将每个映射到其正方形
4. 一次同时处理10个（用sleep模拟）
5. 收集结果

<details>
<summary>🔑 参考答案</summary>

```rust
use futures::stream::{self, StreamExt};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let results: Vec<u64> = stream::iter(1u64..=100)
        // 第 2 步：过滤事件
        .filter(|x| futures::future::ready(x % 2 == 0))
        // 第 3 步：对每个进行平方
        .map(|x| x * x)
        // 第四步：并发处理（模拟async工作）
        .map(|x| async move {
            sleep(Duration::from_millis(50)).await;
            println!("Processed: {x}");
            x
        })
        .buffer_unordered(10) // 10个并发
        // 第五步：收集
        .collect()
        .await;

    println!("Got {} results", results.len());
    println!("Sum: {}", results.iter().sum::<u64>());
}
```

</details>

---

### 练习 6：实现带有超时的 Select

在不使用 `tokio::select!` 或 `tokio::time::timeout` 的情况下，实现一个与截止日期竞争Future 的函数，并在超时时返回 `Either::Left(result)` 或 `Either::Right(())`。

*提示*：基于第 6 章中的 `Select` 组合器和同一章中的 `TimerFuture` 进行构建。

<details>
<summary>🔑 参考答案</summary>

```rust,ignore
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub enum Either<A, B> {
    Left(A),
    Right(B),
}

pub struct Timeout<F> {
    future: F,
    timer: TimerFuture, // 从第 6 章开始
}

impl<F: Future + Unpin> Timeout<F> {
    pub fn new(future: F, duration: Duration) -> Self {
        Timeout {
            future,
            timer: TimerFuture::new(duration),
        }
    }
}

impl<F: Future + Unpin> Future for Timeout<F> {
    type Output = Either<F::Output, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 检查主要Future是否完成
        if let Poll::Ready(val) = Pin::new(&mut self.future).poll(cx) {
            return Poll::Ready(Either::Left(val));
        }

        // 检查定时器是否超时
        if let Poll::Ready(()) = Pin::new(&mut self.timer).poll(cx) {
            return Poll::Ready(Either::Right(()));
        }

        Poll::Pending
    }
}

// 用法：
// match Timeout::new(fetch_data(), Duration::from_secs(5)).await {
//     Either::Left(data) => println!("Got data: {data}"),
//     Either::Right(()) => println!("Timed out!"),
// }
```

**关键要点**：`select`/`timeout` 只是轮询两个 future 并查看哪个先完成。整个异步生态系统是由这个简单的原语构建的：poll、Pending/Ready、Waker。

</details>

***

