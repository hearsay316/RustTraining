# 异步 Rust：从Future到生产

## 演讲者介绍

- Microsoft SCHIE（硅和云硬件基础设施工程）团队首席固件架构师
- 行业资深人士，拥有安全、系统编程（固件、操作系统、虚拟机管理程序）、CPU 和平台架构以及 C++ 系统方面的专业知识
- 2017 年Rust开始编程（@AWS EC2），从那时起就爱上了这门语言

---

Rust 中异步编程的深入指南。与大多数从 `tokio::main` 开始并手动解释内部结构的异步教程不同，本指南从第一性原理（`Future` trait、轮询、状态机）构建理解，然后深入到现实世界的模式、Runtime 选择和生产环境陷阱。

## 这是给谁的
- Rust 可以编写同步 Rust 但发现异步令人困惑的开发人员
- 来自C#、Go、Python或JavaScript的开发人员知道`async/await`但不知道Rust的模型
- 任何被 `Future is not Send`、`Pin<Box<dyn Future>>` 或“为什么我的程序挂起？”困扰的人

## 先决条件

您应该对以下内容感到满意：
- 所有权、借用和生命周期
- trait 和泛型（包括`impl Trait`）
- 使用 `Result<T, E>` 和 `?` 运算符
- 基本多线程（`std::thread::spawn`、`Arc`、`Mutex`）

无需具备异步 Rust 经验。

## 如何使用本书

**第一次线性阅读。** 第 I 至第 III 部分相互构建。每章有：

| 符号 | 意义 |
|--------|---------|
| 🟢 | 初学者——基本概念 |
| 🟡 | 中级——需要前面的章节 |
| 🔴 | 高级——深层内部结构或生产模式 |

每章包括：
- 顶部的 **“您将学到什么”** 块
- **Mermaid 图表** 适合视觉学习者
- 带有隐藏参考答案的**内联练习**
- **关键要点**总结核心思想
- **相关章节的交叉引用**

## 节奏指南

| 章节 | 主题 | 建议时间 | 检查点 |
|----------|-------|----------------|------------|
| 1–5 | 异步如何工作 | 6-8小时 | 您可以解释 `Future`、`Poll`、`Pin`，以及为什么 Rust 没有内置 Runtime |
| 6–10 | 生态系统 | 6-8小时 | 您可以手动构建 future，选择 Runtime，并使用 tokio 的 API |
| 11–13 | 生产异步 | 6-8小时 | 您可以使用流、正确的错误处理和正常关闭来编写生产级异步代码 |
| Capstone | 聊天服务器 | 4-6小时 | 您已经构建了一个集成所有概念的真正的异步应用程序 |

**预计总时间：22–30 小时**

## 如何完成练习

每个内容章节都有一个内联练习。Capstone（第 16 章）将所有内容集成到一个项目中。为了最大限度地学习：

1. **在扩展参考答案之前先尝试练习**——卡住和思考正是学习发生的地方
2. **手动敲代码，不要复制粘贴** — 肌肉记忆对于 Rust 的语法很重要
3. **运行每个示例** — `cargo new async-exercises` 并进行测试

## 目录

### 第一部分：异步如何工作

- [1. 为什么异步在 Rust 中有所不同](ch01-why-async-is-different-in-rust.md) 🟢 — 根本区别：Rust 没有内置 Runtime
- [2. Future trait](ch02-the-future-trait.md) 🟡 — `poll()`、`Waker`，以及使这一切顺利进行的契约
- [3. Poll 的工作原理](ch03-how-poll-works.md) 🟡 — 轮询状态机和最小执行器
- [4. Pin 和 Unpin](ch04-pin-and-unpin.md) 🔴 — 为什么自引用结构需要固定
- [5. 状态机揭示](ch05-the-state-machine-reveal.md) 🟢 — 编译器实际从 `async fn` 生成的内容

### 第二部分：生态系统

- [6. 手工构建 Future](ch06-building-futures-by-hand.md) 🟡 — TimerFuture、Join、Select 从头开始
- [7. 执行器和 Runtime](ch07-executors-and-runtimes.md) 🟡 — tokio、smol、async-std、embassy — 如何选择
- [8. Tokio 深入研究](ch08-tokio-deep-dive.md) 🟡 — Runtime 类型、spawn、通道、同步原语
- [9. 当 Tokio 不合适时](ch09-when-tokio-isnt-the-right-fit.md) 🟡 — LocalSet、FuturesUnordered，与 Runtime 无关的设计
- [10. 异步 trait](ch10-async-traits.md) 🟡 — RPITIT、dyn 调度、trait_variant、异步闭包

### 第三部分：生产异步

- [11. 流和 AsyncIterator](ch11-streams-and-asynciterator.md) 🟡 — 异步迭代，AsyncRead/AsyncWrite，流组合器
- [12. 常见陷阱](ch12-common-pitfalls.md) 🔴 — 9 个生产错误以及如何避免它们
- [13. 生产模式](ch13-production-patterns.md) 🔴 — 优雅关闭、背压、Tower 中间件
- [14. 异步是一种优化，而不是一种架构](ch14-async-is-an-optimization-not-an-architecture.md) 🔴 — Sync 核心/异步 shell，函数着色成本

### 附录

- [总结和参考卡](ch16-summary-and-reference-card.md) — 快速查找表和决策树
- [Capstone 项目：异步聊天服务器](ch17-capstone-project.md) — 构建一个完整的异步应用程序

***


