# Capstone 项目：异步聊天服务器

该项目将书中的模式集成到一个单一的、生产风格的应用程序中。您将使用 tokio、通道、流、正常关闭和正确的错误处理来构建 **多房间异步聊天服务器**。

**预计时间**：4-6 小时 | **难度**：★★★

> **你将练习什么：**
> - `tokio::spawn` 和 `'static` 要求（第 8 章）
> - 通道：`mpsc`用于消息，`broadcast`用于房间，`watch`用于关闭（第8章）
> - 流：从 TCP 连接读取行（第 11 章）
> - 常见陷阱：取消安全、MutexGuard 跨越 `.await`（第 12 章）
> - 生产模式：正常关闭、背压（第 13 章）
> - 可插入后端的异步 trait（第 10 章）

## 问题

构建一个 TCP 聊天服务器，其中：

1. **客户端**通过 TCP 和 join 命名房间连接
2. **消息**广播给同一房间的所有客户
3. **命令**：`/join <room>`、`/nick <name>`、`/rooms`、`/quit`
4. 服务器按 Ctrl+C 正常关闭 — 完成传输中的消息

```mermaid
graph LR
    C1["客户端 1<br/>（Alice）"] -->|TCP| SERVER["聊天服务器"]
    C2["客户端 2<br/>（Bob）"] -->|TCP| SERVER
    C3["客户端 3<br/>（Carol）"] -->|TCP| SERVER

    SERVER --> R1["#general<br/>broadcast 通道"]
    SERVER --> R2["#rust<br/>broadcast 通道"]

    R1 -->|消息| C1
    R1 -->|消息| C2
    R2 -->|消息| C3

    CTRL["Ctrl+C"] -->|watch| SERVER

    style SERVER fill:#e8f4f8,stroke:#2980b9,color:#000
    style R1 fill:#d4efdf,stroke:#27ae60,color:#000
    style R2 fill:#d4efdf,stroke:#27ae60,color:#000
    style CTRL fill:#fadbd8,stroke:#e74c3c,color:#000
```

## 第 1 步：基本 TCP 接受循环

从接受连接并回显线路的服务器开始：

```rust
// 小白提示：这段代码演示【第 1 步：基本 TCP 接受循环】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Chat server listening on :8080");

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
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let _ = writer.write_all(line.as_bytes()).await;
                    }
                }
            }
            println!("[{addr}] Disconnected");
        });
    }
}
```

**你的工作**：验证它是否可以编译并与 `telnet localhost 8080` 一起使用。

## 步骤 2：带有广播频道的房间状态

每个房间都是`broadcast::Sender`。房间中的所有客户端都订阅接收消息。

```rust
// 小白提示：这段代码演示【步骤 2：带有广播频道的房间状态】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

type RoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;

fn get_or_create_room(rooms: &mut HashMap<String, broadcast::Sender<String>>, name: &str) -> broadcast::Sender<String> {
    rooms.entry(name.to_string())
        .or_insert_with(|| {
            let (tx, _) = broadcast::channel(100); // 100 条消息的缓冲区
            tx
        })
        .clone()
}
```

**您的工作**：实施房间状态，以便：
- 客户从`#general`开始
- `/join <room>` 切换房间（取消订阅旧房间，订阅新房间）
- 消息将广播给发送者当前房间中的所有客户端

<details>
<summary>💡提示 — 客户端任务结构</summary>

每个客户端任务需要两个并发循环：
1. **从 TCP 读取** → 解析命令或广播到房间
2. **从广播接收器读取** → 写入 TCP

使用 `tokio::select!` 运行两者：

```rust
// 小白提示：这段代码演示【步骤 2：带有广播频道的房间状态】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
loop {
    tokio::select! {
        // 客户端发来一行
        result = reader.read_line(&mut line) => {
            match result {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // 解析命令或广播消息
                }
            }
        }
        // 收到房间广播
        result = room_rx.recv() => {
            match result {
                Ok(msg) => {
                    let _ = writer.write_all(msg.as_bytes()).await;
                }
                Err(_) => break,
            }
        }
    }
}
```

</details>

## 第 3 步：命令

实现命令协议：

| 命令 | 行动 |
|---------|--------|
| `/join <room>` | 离开当前房间，join新房间，在双方公告 |
| `/nick <name>` | 更改显示名称 |
| `/rooms` | 列出所有活跃房间和成员数量 |
| `/quit` | 优雅地断开连接 |
| 还要别的吗 | 作为聊天消息广播 |

**你的工作**：解析输入行中的命令。对于 `/rooms`，您需要从 `RoomMap` 读取 — 使用 `RwLock::read()` 以避免阻塞其他客户端。

## 第 4 步：正常关机

添加 Ctrl+C 处理，以便服务器：
1. 停止接受新连接
2. 向所有房间发送“服务器正在关闭...”
3. 等待传输中的消息耗尽
4. 干净地退出

```rust
// 小白提示：这段代码演示【第 4 步：正常关机】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
use tokio::sync::watch;

let (shutdown_tx, shutdown_rx) = watch::channel(false);

// 在接受循环中：
loop {
    tokio::select! {
        result = listener.accept() => {
            let (socket, addr) = result?;
            // 使用 shutdown_rx.clone() 生成客户端任务
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Shutdown signal received");
            shutdown_tx.send(true)?;
            break;
        }
    }
}
```

**您的工作**：将 `shutdown_rx.changed()` 添加到每个客户端的 `select!` 循环，以便客户端在收到关闭信号时退出。

## 第 5 步：错误处理和边缘情况

对服务器进行生产强化：

1. **滞后接收者**：如果慢速客户端错过消息，则`broadcast::recv()`返回`RecvError::Lagged(n)`。优雅地处理它（记录+继续，不要崩溃）。
2. **昵称验证**：拒绝空或太长的昵称。
3. **背压**：广播通道缓冲区有界（100）。如果客户端无法跟上，他们会收到 `Lagged` 错误。
4. **超时**：断开空闲时间超过 5 分钟的客户端。

```rust
// 小白提示：这段代码演示【第 5 步：错误处理和边缘情况】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
use tokio::time::{timeout, Duration};

// 给读取操作包一层超时：
match timeout(Duration::from_secs(300), reader.read_line(&mut line)).await {
    Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break, // EOF、错误或超时
    Ok(Ok(_)) => { /* 处理这一行 */ }
}
```

## 第6步：集成测试

编写一个启动服务器、连接两个客户端并验证消息传递的测试：

```rust
// 小白提示：这段代码演示【第6步：集成测试】。先看类型/函数签名，再看 .await、poll、spawn 等关键调用怎样推动异步任务。
#[tokio::test]
async fn two_clients_can_chat() {
    // 在后台启动服务器
    let server = tokio::spawn(run_server("127.0.0.1:0")); // 端口 0 = 由 OS 自动选择

    // 连接两个客户端
    let mut client1 = TcpStream::connect(addr).await.unwrap();
    let mut client2 = TcpStream::connect(addr).await.unwrap();

    // 客户端 1 发送消息
    client1.write_all(b"Hello from client 1\n").await.unwrap();

    // 客户端 2 应该收到消息
    let mut buf = vec![0u8; 1024];
    let n = client2.read(&mut buf).await.unwrap();
    let msg = String::from_utf8_lossy(&buf[..n]);
    assert!(msg.contains("Hello from client 1"));
}
```

## 评价标准

| 标准 | 目标 |
|-----------|--------|
| 并发性 | 多个房间多个客户端，无阻塞 |
| 正确性 | 消息仅发送给同一房间的客户 |
| 优雅关机 | Ctrl+C 耗尽消息并干净退出 |
| 错误处理 | 接收器滞后、断线、超时处理 |
| 代码组织 | 干净的分离：接受循环、客户端任务、房间状态 |
| 测试 | 至少2次集成测试 |

## 扩展想法

基本聊天服务器工作后，请尝试以下增强功能：

1. **持久历史记录**：存储每个房间的最后 N 条消息；向新加入者重播
2. **WebSocket 支持**：使用 `tokio-tungstenite` 接受 TCP 和 WebSocket 客户端
3. **速率限制**：使用 `tokio::time::Interval` 限制每个客户端每秒的消息数
4. **指标**：通过 `prometheus` crate 跟踪连接的客户端、消息/秒、房间数
5. **TLS**：为加密连接添加`tokio-rustls`

***
