# 11. Stream 与 AsyncIterator

> **你将学到什么：**
> - `Stream` trait：异步产生多个值
> - 创建 Stream：`stream::iter`、`async_stream`、`unfold`
> - Stream 组合器：`map`、`filter`、`buffer_unordered`、`fold`
> - 异步 I/O trait：`AsyncRead`、`AsyncWrite`、`AsyncBufRead`

## Stream Trait 概述

`Stream` 之于 `Iterator`，正如 `Future` 之于单个值——它异步地产生多个值：

```rust
// ============================================================
// 核心概念：Stream 是 Iterator 的异步对应物
// ============================================================
// Iterator::next()     → 同步，返回 Option<Item>（立即可得）
// Stream::poll_next()  → 异步，返回 Poll<Option<Item>>
//    Poll::Pending     → 值尚未就绪，稍后再问
//    Poll::Ready(Some) → 下一个值就绪
//    Poll::Ready(None) → Stream 已结束（等价于 Iterator 的 None）
// 设计理由：Stream 允许在多个值之间出现异步等待，
//   不像 Iterator 假设所有值立即可得。
// ============================================================

// std::iter::Iterator（同步，多个值）
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    // → 立即可得或结束
}

// futures::Stream（异步，多个值）
trait Stream {
    type Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
    //               ^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^
    //               Pin 固定内存位置   Context 用于注册 waker
}
```

```mermaid
graph LR
    subgraph "同步"
        VAL["值<br/>（T）"]
        ITER["Iterator<br/>（多个 T）"]
    end

    subgraph "异步"
        FUT["Future<br/>（异步单个 T）"]
        STREAM["Stream<br/>（异步多个 T）"]
    end

    VAL -->|"异步化"| FUT
    ITER -->|"异步化"| STREAM
    VAL -->|"多值化"| ITER
    FUT -->|"多值化"| STREAM

    style VAL fill:#e3f2fd,color:#000
    style ITER fill:#e3f2fd,color:#000
    style FUT fill:#c8e6c9,color:#000
    style STREAM fill:#c8e6c9,color:#000
```

### 创建 Stream

```rust
// ============================================================
// 核心概念：五种创建 Stream 的方式
// ============================================================
// 1. stream::iter  → 从已知列表/迭代器创建（值立即可得）
// 2. async_stream! → 宏语法，使用 yield 关键字（最直观）
// 3. IntervalStream → 从定时器创建周期性信号流
// 4. ReceiverStream → 从 mpsc 通道接收端创建
// 5. stream::unfold → 从状态机创建（函数式风格）
// ============================================================

use futures::stream::{self, StreamExt};
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::IntervalStream;

// 1. 从迭代器创建
let s = stream::iter(vec![1, 2, 3]);
// → 立即产生 1, 2, 3（所有值立即可得）

// 2. 从 async 生成器创建（使用 async-stream crate）
// Cargo.toml: async-stream = "0.3"
use async_stream::stream;

fn countdown(from: u32) -> impl futures::Stream<Item = u32> {
    stream! {
        // ↓ 在 async 块内使用 yield 产出值
        for i in (0..=from).rev() {
            tokio::time::sleep(Duration::from_millis(500)).await;
            yield i; // → 产出当前值，挂起直到消费者请求下一个
        }
    }
}

// 3. 从 tokio 定时器创建
let tick_stream = IntervalStream::new(interval(Duration::from_secs(1)));
// → 每 1 秒产生一个 () 值

// 4. 从通道接收端创建
let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);
let rx_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
// → 每个 send 的消息变成一个 Stream item

// 5. 从 unfold 创建（状态机风格）
let s = stream::unfold(0u32, |state| async move {
    //                       ^^^^^ 当前状态值
    if state >= 5 {
        None // → Stream 结束
    } else {
        let next = state + 1;
        Some((state, next)) // → (产出的值, 新状态)
    }
});
// 产出序列：0, 1, 2, 3, 4
```

### 消费 Stream

```rust
// ============================================================
// 核心概念：Stream 消费模式
// ============================================================
// 类似 Iterator，Stream 有一系列组合器（combinator）。
// 关键差异：所有消费方法都是 async——需要 .await。
//
// 关键 API：
//   .for_each()          → 对每个元素执行异步操作
//   .map().collect()     → 转换每个元素并收集到集合
//   .filter()            → 异步过滤（注意闭包返回 Future<bool>）
//   .buffer_unordered(N) → 最多 N 个元素并发处理，按完成顺序返回
//   .take(N) / .skip(N)  → 类似 Iterator 的对应方法
// ============================================================

use futures::stream::{self, StreamExt};

async fn stream_examples() {
    let s = stream::iter(vec![1, 2, 3, 4, 5]);

    // for_each：对每个元素执行异步操作
    s.for_each(|x| async move {
        println!("{x}");
        // → 逐一处理每个元素
    }).await;

    // map + collect：转换并收集
    let doubled: Vec<i32> = stream::iter(vec![1, 2, 3])
        .map(|x| x * 2)     // 注意：map 闭包是同步的
        .collect()          // → 等待所有元素并收集到 Vec
        .await;

    // filter：异步过滤（闭包返回 Future<bool>）
    let evens: Vec<i32> = stream::iter(1..=10)
        .filter(|x| futures::future::ready(x % 2 == 0))
        //          ↑ 此处用 ready 包装同步判断为 Future
        //            实际项目中这里可以是 async 条件判断
        .collect()
        .await;

    // buffer_unordered —— 同时处理最多 N 个元素
    // ⚠️ 这是 Stream 最强大的并发工具之一
    let results: Vec<_> = stream::iter(vec!["url1", "url2", "url3"])
        .map(|url| async move {
            // → 每个 map 返回一个 Future（模拟 HTTP 请求）
            tokio::time::sleep(Duration::from_millis(100)).await;
            format!("response from {url}")
        })
        .buffer_unordered(10) // 最多 10 个并发请求同时进行
        .collect()            // → 按完成顺序收集结果
        .await;

    // take、skip、zip、chain —— 与 Iterator 一致
    let first_three: Vec<i32> = stream::iter(1..=100)
        .take(3)  // → 只取前 3 个元素
        .collect()
        .await;
}
```

### 与 C# IAsyncEnumerable 的比较

| 特性 | Rust `Stream` | C# `IAsyncEnumerable<T>` |
|---------|--------------|--------------------------|
| **语法** | `stream! { yield x; }` | `await foreach` / `yield return` |
| **取消** | 丢弃 Stream 即可 | `CancellationToken` |
| **背压（backpressure）** | 消费者控制 poll 频率 | 消费者控制 `MoveNextAsync` |
| **内置** | 否（需要 `futures` crate） | 是（自 C# 8.0 起） |
| **组合器** | `.map()`、`.filter()`、`.buffer_unordered()` | LINQ + `System.Linq.Async` |
| **错误处理** | `Stream<Item = Result<T, E>>` | 在异步迭代器内部抛出 |

```rust
// ============================================================
// 对比：Rust Stream vs C# IAsyncEnumerable
// ============================================================
// 两者都能异步产生多个值，但实现差异显著：
// Rust  → 基于 poll 模型，消费者驱动（pull-based）
//         任何值准备就绪时 poll_next() 返回 Poll::Ready(Some)
// C#    → 基于 yield return，语言集成更深
//
// ⚠️ 注意：包含 ? 运算符时需使用 try_stream! 而非 stream!
//   stream! 不传播错误——你需手动处理 Result
//   try_stream! 遇到 Err(e) 时自动 yield Err(e) 并结束
// ============================================================

// Rust：从数据库逐行读取的 Stream
// ⚠️ 注意：在 ? 运算符体内使用 try_stream!（而不是 stream!）
fn get_users(db: &Database) -> impl Stream<Item = Result<User, DbError>> + '_ {
    try_stream! {
        let mut cursor = db.query("SELECT * FROM users").await?;
        //                                                      ↑ ? 传播错误
        while let Some(row) = cursor.next().await {
            yield User::from_row(row?);
            //                        ↑ ? 传播错误
        }
    }
}

// 消费：
let mut users = pin!(get_users(&db));
//               ^^^^^ pin! 将 Stream 固定在栈上（poll_next 需要 Pin）
while let Some(result) = users.next().await {
    match result {
        Ok(user) => println!("{}", user.name),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

```csharp
// C# 等价写法（对照参考）：
async IAsyncEnumerable<User> GetUsers() {
    await using var reader = await db.QueryAsync("SELECT * FROM users");
    while (await reader.ReadAsync()) {
        yield return User.FromRow(reader);
    }
}

// 消费：
await foreach (var user in GetUsers()) {
    Console.WriteLine(user.Name);
}
```

<details>
<summary><strong>练习：构建异步统计聚合器</strong>（点击展开）</summary>

**挑战**：给定传感器读数 Stream `Stream<Item = f64>`，编写一个异步函数消费该 Stream 并返回 `(count, min, max, average)`。使用 `StreamExt` 组合器——不要仅仅收集到 Vec 中。

*提示*：使用 `.fold()` 在 Stream 中累积状态。

<details>
<summary>参考答案</summary>

```rust
// ============================================================
// 练习：使用 fold() 在 Stream 上做状态累积
// ============================================================
// fold() 类似 Iterator::fold()，但 accumulator 闭包是 async。
// 初始状态传入 Stats 的默认实例，每收一个值更新一次累积状态。
// 这种方式的优势：O(1) 内存——不需要先把整个 Stream 收集到 Vec。
// 设计理由：对于无界或巨大的数据流，fold 是唯一可行的方式。
// ============================================================

use futures::stream::{self, StreamExt};

#[derive(Debug)]
struct Stats {
    count: usize,
    min: f64,
    max: f64,
    sum: f64,
}

impl Stats {
    fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

async fn compute_stats<S: futures::Stream<Item = f64>>(stream: S) -> Stats {
    stream
        .fold(
            // 初始累积状态
            Stats { count: 0, min: f64::INFINITY, max: f64::NEG_INFINITY, sum: 0.0 },
            // |acc, value| async move  → 每收到一个值，更新累积状态
            |mut acc, value| async move {
                acc.count += 1;
                acc.min = acc.min.min(value);  // ← f64 的 min() 方法
                acc.max = acc.max.max(value);
                acc.sum += value;
                acc // → 返回更新后的状态给下一次 fold 调用
            },
        )
        .await
    // → 最终累积状态即为完整结果
}

#[tokio::test]
async fn test_stats() {
    let readings = stream::iter(vec![23.5, 24.1, 22.8, 25.0, 23.9]);
    let stats = compute_stats(readings).await;

    assert_eq!(stats.count, 5);
    assert!((stats.min - 22.8).abs() < f64::EPSILON);
    assert!((stats.max - 25.0).abs() < f64::EPSILON);
    assert!((stats.average() - 23.86).abs() < 0.01);
}
```

**关键要点**：Stream 的组合器如 `.fold()` 逐个处理元素而不收集到内存中——这对于处理大型或无界数据流至关重要。

</details>
</details>

### 异步 I/O Trait：AsyncRead、AsyncWrite、AsyncBufRead

如同 `std::io::Read`/`Write` 是同步 I/O 的基础，它们的异步对应 trait 也是异步 I/O 的基石。这些 trait 由 `tokio::io` 提供（或用 `futures::io` 编写与运行时无关的代码）：

```rust
// ============================================================
// 核心概念：异步 I/O trait 的设计
// ============================================================
// 所有 poll_* 方法返回 Poll<io::Result<...>>：
//   Poll::Pending    → I/O 尚未就绪，waker 已注册，不阻塞线程
//   Poll::Ready(Ok)  → 操作完成
//   Poll::Ready(Err) → I/O 错误
//
// tokio 版本与 futures 版本的关键差异：
//   tokio::AsyncRead 使用 ReadBuf（安全处理未初始化内存）
//   futures::AsyncRead 使用 &mut [u8]（更简单，但需要预初始化）
// ============================================================

// tokio::io：std::io trait 的异步版本

/// 从源异步读取字节
pub trait AsyncRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,  // Tokio 的未初始化内存安全包装
    ) -> Poll<io::Result<()>>;
}

/// 异步向接收方写入字节
pub trait AsyncWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>; // → 返回实际写入的字节数

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
    // → 刷新中间缓冲区，确保数据发送到目标

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
    // → 优雅关闭写入端（如 TCP FIN）
}

/// 带缓冲的行读取支持
pub trait AsyncBufRead: AsyncRead {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>>;
    // → 填充内部缓冲区，返回可读的字节切片（不消费）
    fn consume(self: Pin<&mut Self>, amt: usize);
    // → 消费已读取的 amt 字节，丢弃缓冲区中对应部分
}
```

**实际使用中**，你很少直接调用这些 `poll_*` 方法。相反，使用扩展 trait `AsyncReadExt` 和 `AsyncWriteExt`，它们提供 `.await` 友好的辅助方法：

```rust
// ============================================================
// 核心概念：AsyncReadExt / AsyncWriteExt 提供高阶方法
// ============================================================
// 扩展 trait 将底层的 poll_* 方法封装为 async fn，
// 让你能用 .await 自然地编写异步 I/O 代码。
// 常用高阶方法：
//   read_to_end()    → 读取所有字节直到 EOF
//   read_to_string() → 读取所有字节并解析为 UTF-8 字符串
//   write_all()      → 写入所有字节（处理部分写入）
//   lines()          → 逐行读取（需要 BufReader）
// ============================================================

use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt};
use tokio::net::TcpStream;
use tokio::io::BufReader;

async fn io_examples() -> tokio::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;

    // AsyncWriteExt：write_all 确保完整写入
    stream.write_all(b"GET / HTTP/1.0\r\n\r\n").await?;
    // → 写入 HTTP 请求；await 等待所有字节发送完毕

    // AsyncReadExt：read_to_end 读取所有响应
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    // → 不断读取直到 EOF，追加到 response 中

    // AsyncBufReadExt：逐行读取文件
    let file = tokio::fs::File::open("config.txt").await?;
    let reader = BufReader::new(file);     // 包装为缓冲读取器
    let mut lines = reader.lines();        // → 返回行迭代器
    while let Some(line) = lines.next_line().await? {
        println!("{line}");
    }

    Ok(())
}
```

**实现自定义异步 I/O**——在原始 TCP 上封装协议：

```rust
// ============================================================
// 核心概念：基于 AsyncRead/AsyncWrite 实现帧协议
// ============================================================
// FramedStream 在原始字节流上实现长度前缀协议：
//   [4 字节长度（u32 LE）][N 字节载荷]
// read_u32() 和 read_exact() 来自 AsyncReadExt，
// write_u32() 和 write_all() 来自 AsyncWriteExt。
// 设计理由：这是构建自定义应用层协议（如 Redis RESP、
// gRPC 长度前缀帧等）的标准模式。
// ⚠️ 注意：T 上同时约束 AsyncRead + AsyncReadExt + Unpin，
//   因为 AsyncReadExt 的方法需要 Unpin 才能被 .await。
// ============================================================

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

/// 长度前缀协议：[u32 长度][载荷字节]
struct FramedStream<T> {
    inner: T,
}

impl<T: AsyncRead + AsyncReadExt + Unpin> FramedStream<T> {
    /// 读取完整的一帧
    async fn read_frame(&mut self) -> tokio::io::Result<Vec<u8>>
    {
        // 读取 4 字节长度前缀（u32 LE 编码）
        let len = self.inner.read_u32().await? as usize;
        // → 知道接下来要读多少字节

        // 准确读取 len 字节
        let mut payload = vec![0u8; len];
        self.inner.read_exact(&mut payload).await?;
        // → 必须读满 len 字节，否则返回 UnexpectedEof 错误
        Ok(payload)
    }
}

impl<T: AsyncWrite + AsyncWriteExt + Unpin> FramedStream<T> {
    /// 写出一完整帧
    async fn write_frame(&mut self, data: &[u8]) -> tokio::io::Result<()>
    {
        self.inner.write_u32(data.len() as u32).await?; // 写入长度前缀
        self.inner.write_all(data).await?;               // 写入载荷
        self.inner.flush().await?;                        // 确保发送到网络
        Ok(())
    }
}
```

| 同步 Trait | 异步 Trait (tokio) | 异步 Trait (futures) | 扩展 Trait |
|-----------|--------------------|-----------------------|----------------|
| `std::io::Read` | `tokio::io::AsyncRead` | `futures::io::AsyncRead` | `AsyncReadExt` |
| `std::io::Write` | `tokio::io::AsyncWrite` | `futures::io::AsyncWrite` | `AsyncWriteExt` |
| `std::io::BufRead` | `tokio::io::AsyncBufRead` | `futures::io::AsyncBufRead` | `AsyncBufReadExt` |
| `std::io::Seek` | `tokio::io::AsyncSeek` | `futures::io::AsyncSeek` | `AsyncSeekExt` |

> **tokio 与 futures I/O trait 的差异**：它们相似但不完全相同——tokio 的 `AsyncRead` 使用 `ReadBuf`（安全处理未初始化的内存），而 `futures::AsyncRead` 使用 `&mut [u8]`。使用 `tokio_util::compat` 在它们之间进行转换。

> **复制工具**：`tokio::io::copy(&mut reader, &mut writer)` 是 `std::io::copy` 的异步等价物——对代理服务器或文件传输很有用。`tokio::io::copy_bidirectional` 同时双向复制（全双工代理）。

<details>
<summary><strong>练习：构建异步行计数器</strong>（点击展开）</summary>

**挑战**：编写一个异步函数，接受任意 `AsyncBufRead` 源并返回非空行数。它应该适用于文件、TCP 流或任何缓冲读取器。

*提示*：使用 `AsyncBufReadExt::lines()` 并计数 `!line.is_empty()` 的行。

<details>
<summary>参考答案</summary>

```rust
// ============================================================
// 练习：基于 AsyncBufRead trait 编写通用 I/O 函数
// ============================================================
// count_non_empty_lines 接受泛型 R: AsyncBufRead + Unpin，
// 因此可复用给文件、TCP 流、管道、内存缓冲区等。
// Unpin 约束是必需的——lines() 方法返回的 LineReader
// 内部使用了 tokio 的辅助类型，这些类型要求 Unpin。
// ============================================================

use tokio::io::AsyncBufReadExt;

async fn count_non_empty_lines<R: tokio::io::AsyncBufRead + Unpin>(
    reader: R,
) -> tokio::io::Result<usize> {
    let mut lines = reader.lines();
    // → lines() 创建行流，每次 next_line() 读取一行
    let mut count = 0;
    while let Some(line) = lines.next_line().await? {
        //                        ↑ 异步读取下一行，? 传播 I/O 错误
        if !line.is_empty() {
            count += 1;
        }
    }
    Ok(count) // → 返回非空行计数
}

// 适用于任何 AsyncBufRead：
// let file = tokio::io::BufReader::new(tokio::fs::File::open("data.txt").await?);
// let count = count_non_empty_lines(file).await?;
//
// let tcp = tokio::io::BufReader::new(TcpStream::connect("...").await?);
// let count = count_non_empty_lines(tcp).await?;
//
// let mem = tokio::io::BufReader::new(std::io::Cursor::new(b"hello\n\nworld\n"));
// let count = count_non_empty_lines(mem).await?;
```

**关键要点**：通过面向 `AsyncBufRead` trait 编程而非具体类型，你的 I/O 代码可以在文件、套接字、管道甚至内存缓冲区（`tokio::io::BufReader::new(std::io::Cursor::new(data))`）之间复用。

</details>
</details>

> **关键要点 -- Stream 与 AsyncIterator**
> - `Stream` 是 `Iterator` 的异步等价物——产生 `Poll::Ready(Some(item))` 或 `Poll::Ready(None)`
> - `.buffer_unordered(N)` 同时处理 N 个 Stream 元素——Stream 最关键的并发工具
> - `async_stream::stream!` 是创建自定义 Stream 的最简单方式（使用 `yield`）
> - `AsyncRead`/`AsyncBufRead` 支持跨文件、套接字和管道的通用、可复用 I/O 代码

> **另请参阅：** [第 9 章 -- 当 Tokio 不合适时](ch09-when-tokio-isnt-the-right-fit.md) 了解 `FuturesUnordered`（相关模式），[第 13 章 -- 生产模式](ch13-production-patterns.md) 了解有界通道的背压

***

