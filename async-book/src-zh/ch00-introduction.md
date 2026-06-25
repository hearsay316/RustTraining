# 异步（async）Rust：从 Future 到生产

## 演讲者介绍

- Microsoft SCHIE（硅与云硬件基础设施工程）团队首席固件架构师
- 行业资深人士，拥有安全、系统编程（固件、操作系统、虚拟机管理程序）、CPU 和平台架构以及 C++ 系统方面的专业知识
- 2017 年开始使用 Rust 编程（@AWS EC2），从那时起就爱上了这门语言

---

Rust 异步编程深入指南。与大多数从 `tokio::main` 开始、然后手工解释内部机制的异步教程不同，本指南从第一性原理出发（`Future` trait、轮询、状态机（state machine）），逐步构建理解，最后深入到真实世界的模式、Runtime 选型和生产环境陷阱。

## 目标读者
- 能写同步 Rust 但对异步感到困惑的开发者
- 来自 C#、Go、Python 或 JavaScript，熟悉 `async/await` 但不了解 Rust 模型的开发者
- 被 `Future is not Send`、`Pin<Box<dyn Future>>` 或"为什么我的程序卡死了？"折磨过的人

## 先决条件

你应该已经熟悉以下内容：
- 所有权、借用和生命周期
- trait 和泛型（包括 `impl Trait`）
- 使用 `Result<T, E>` 和 `?` 操作符
- 基础多线程（`std::thread::spawn`、`Arc`、`Mutex`）

不需要异步 Rust 经验。

## 如何使用本书

**第一遍建议线性阅读。** 第 I 至第 III 部分相互依赖。每章标有难度等级：

| 符号 | 含义 |
|--------|---------|
| 🟢 | 初学者——基础概念 |
| 🟡 | 中级——依赖前序章节 |
| 🔴 | 高级——深层内部机制或生产模式 |

每章包含：
- 顶部 **"你将学到什么"** 知识块
- 面向视觉学习者的 **Mermaid 图表**
- 带折叠参考答案的**内联练习**
- 总结核心思想的**关键要点**
- **相关章节交叉引用**

## 进度指南

| 章节 | 主题 | 建议时长 | 学习检查点 |
|----------|-------|----------------|------------|
| 1–5 | 异步如何工作 | 6–8 小时 | 你能解释 `Future`、`Poll`、`Pin`，以及为什么 Rust 没有内置 Runtime |
| 6–10 | 生态系统 | 6–8 小时 | 你能手工构造 Future，选择 Runtime，使用 tokio 的主要 API |
| 11–13 | 生产级异步 | 6–8 小时 | 你能用流、正确的错误处理和优雅关闭写出生产级异步代码 |
| Capstone | 聊天服务器 | 4–6 小时 | 你构建了一个整合所有概念的真正异步应用 |

**预估总时长：22–30 小时**

## 如何完成练习

每个内容章节都配备内联练习。Capstone（第 16 章）将所有内容整合到一个项目中。为了最大化学习效果：

1. **展开参考答案之前先自己尝试**——卡住并思考正是学习发生的地方
2. **亲手敲代码，不要复制粘贴**——肌肉记忆对 Rust 语法很重要
3. **运行每个示例**——`cargo new async-exercises` 并实际测试

## 目录

### 第一部分：异步如何工作

- [1. 为什么 Rust 的异步与众不同](ch01-why-async-is-different-in-rust.md) 🟢 — 根本区别：Rust 没有内置 Runtime
- [2. Future trait](ch02-the-future-trait.md) 🟡 — `poll()`、`Waker`，以及让一切正常运转的契约
- [3. Poll 的工作原理](ch03-how-poll-works.md) 🟡 — 轮询状态机和最小执行器
- [4. Pin 和 Unpin](ch04-pin-and-unpin.md) 🔴 — 为什么自引用结构需要固定
- [5. 状态机揭秘](ch05-the-state-machine-reveal.md) 🟢 — 编译器从 `async fn` 实际生成的内容

### 第二部分：生态系统

- [6. 手工构造 Future](ch06-building-futures-by-hand.md) 🟡 — 从零实现 TimerFuture、Join、Select
- [7. 执行器和 Runtime](ch07-executors-and-runtimes.md) 🟡 — tokio、smol、async-std、embassy 如何选择
- [8. Tokio 深入研究](ch08-tokio-deep-dive.md) 🟡 — Runtime 类型、spawn、通道、同步原语
- [9. Tokio 不适合的场景](ch09-when-tokio-isnt-the-right-fit.md) 🟡 — LocalSet、FuturesUnordered，与 Runtime 无关的设计
- [10. 异步 trait](ch10-async-traits.md) 🟡 — RPITIT、dyn 分发、trait_variant、异步闭包

### 第三部分：生产级异步

- [11. 流和 AsyncIterator](ch11-streams-and-asynciterator.md) 🟡 — 异步迭代、AsyncRead/AsyncWrite、流组合器
- [12. 常见陷阱](ch12-common-pitfalls.md) 🔴 — 9 个生产环境中的典型错误及规避方法
- [13. 生产模式](ch13-production-patterns.md) 🔴 — 优雅关闭、背压、Tower 中间件
- [14. 异步是一种优化，不是一种架构](ch14-async-is-an-optimization-not-an-architecture.md) 🔴 — 同步核心/异步外壳、函数着色成本

### 附录

- [总结与参考卡](ch16-summary-and-reference-card.md) — 快速查找表和决策树
- [Capstone 项目：异步聊天服务器](ch17-capstone-project.md) — 构建一个完整的异步应用

***
