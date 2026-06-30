# Rust 模式与工程实践指南

## 讲师简介

- 微软 SCHIE（硅与云硬件基础设施工程，Silicon and Cloud Hardware Infrastructure Engineering）团队首席固件架构师
- 行业资深专家，擅长安全、系统编程（固件、操作系统、虚拟机监控器）、CPU 与平台架构，以及 C++ 系统开发
- 2017 年（@AWS EC2）开始用 Rust 编程，从此爱上了这门语言

---

这是一本面向中高级开发者的 Rust 模式实战指南，聚焦真实代码库中常见的模式。本书不是语言教程——它假设你已经能编写基本的 Rust 代码，并希望进一步提升。每一章聚焦一个概念，解释何时以及为何使用它，并提供可编译的示例和内联练习。

## 适合的读者

- 已经读完《Rust 程序设计语言》（*The Rust Programming Language*），但在"我到底该怎么设计这个？"上感到困惑的开发者
- 正在将生产系统从 C++/C# 迁移到 Rust 的工程师
- 任何在泛型、trait 约束（trait bound）或生命周期（lifetime）错误上遇到过瓶颈，并希望获得一套系统化工具箱的人

## 前置知识

开始之前，你应该对以下内容比较熟悉：
- 所有权（ownership）、借用（borrowing）和生命周期（基础级别）
- 枚举（enum）、模式匹配（pattern matching）以及 `Option`/`Result`
- 结构体（struct）、方法以及基础 trait（`Display`、`Debug`、`Clone`）
- Cargo 基础：`cargo build`、`cargo test`、`cargo run`

## 如何使用本书

### 难度标识

每一章都标注了难度等级：

| 符号 | 等级 | 含义 |
|--------|-------|---------|
| 🟢 | 基础 | 每个 Rust 开发者都需要的核心概念 |
| 🟡 | 中级 | 生产代码库中常用的模式 |
| 🔴 | 高级 | 深入的语言机制——按需查阅 |

### 进度指南

| 章节 | 主题 | 建议用时 | 检查点 |
|----------|-------|----------------|------------|
| **第一部分：类型层面的模式** | | | |
| 1. 泛型 🟢 | 单态化、const 泛型、`const fn` | 1–2 小时 | 能解释何时 `dyn Trait` 优于泛型 |
| 2. Trait 🟡 | 关联类型、GAT、blanket 实现、vtable | 3–4 小时 | 能设计带关联类型的 trait |
| 3. Newtype 与类型状态 🟡 | 零开销安全、编译期状态机 | 2–3 小时 | 能构建类型状态的 builder 模式 |
| 4. PhantomData 🔴 | 生命周期标记、变型、drop 检查 | 2–3 小时 | 能解释为何 `PhantomData<fn(T)>` 不同于 `PhantomData<T>` |
| **第二部分：并发与运行时** | | | |
| 5. 通道 🟢 | `mpsc`、crossbeam、`select!`、actor | 1–2 小时 | 能实现基于通道的工作线程池 |
| 6. 并发 🟡 | 线程、rayon、Mutex、RwLock、原子操作 | 2–3 小时 | 能为具体场景选择正确的同步原语 |
| 7. 闭包 🟢 | `Fn`/`FnMut`/`FnOnce`、组合子 | 1–2 小时 | 能编写接受闭包的高阶函数 |
| 8. 函数式 vs 命令式 🟡 | 组合子、迭代器适配器、函数式模式 | 2–3 小时 | 能解释函数式风格何时优于命令式 |
| 9. 智能指针 🟡 | Box、Rc、Arc、RefCell、Cow、Pin | 2–3 小时 | 能解释每种智能指针的适用场景 |
| **第三部分：系统与生产实践** | | | |
| 10. 错误处理 🟢 | thiserror、anyhow、`?` 运算符 | 1–2 小时 | 能设计错误类型层级 |
| 11. 序列化 🟡 | serde、零拷贝、二进制数据 | 2–3 小时 | 能编写自定义 serde 反序列化器 |
| 12. Unsafe 🔴 | 超能力、FFI、UB 陷阱、分配器 | 2–3 小时 | 能用健全的安全 API 封装 unsafe 代码 |
| 13. 宏 🟡 | `macro_rules!`、过程宏、`syn`/`quote` | 2–3 小时 | 能用 `tt` 递归（munching）编写声明式宏 |
| 14. 测试 🟢 | 单元/集成/文档测试、proptest、criterion | 1–2 小时 | 能搭建基于属性的测试 |
| 15. API 设计 🟡 | 模块布局、人体工学 API、feature flag | 2–3 小时 | 能运用"解析而非验证"模式 |
| 16. Async 🔴 | Future、Tokio、常见陷阱 | 1–2 小时 | 能识别 async 反模式 |
| **附录** | | | |
| 参考卡片 | 快速查阅 trait 约束、生命周期、模式 | 按需 | — |
| 综合项目 | 类型安全的任务调度器 | 4–6 小时 | 提交一个可运行的实现 |

**总预计时间**：完整学习并完成练习约需 30–45 小时。

### 完成练习

每章末尾都有一个动手练习。为了获得最佳学习效果：

1. **先自己尝试**——在查看答案前至少花 15 分钟
2. **亲手敲代码**——不要复制粘贴；亲手输入能建立肌肉记忆
3. **修改答案**——添加功能、改变约束、故意破坏某些东西
4. **查看交叉引用**——大多数练习综合了多章的模式

综合项目（附录）将全书各章的模式整合为一个完整的生产级系统。

## 目录

### 第一部分：类型层面的模式

**[1. 泛型——全貌](ch01-generics-the-full-picture.md)** 🟢
单态化、代码膨胀的权衡、泛型 vs 枚举 vs trait 对象、const 泛型、`const fn`。

**[2. 深入理解 Trait](ch02-traits-in-depth.md)** 🟡
关联类型、GAT、blanket 实现、marker trait、vtable、HRTB、扩展 trait、枚举分发。

**[3. Newtype 与类型状态模式](ch03-the-newtype-and-type-state-patterns.md)** 🟡
零开销类型安全、编译期状态机、builder 模式、配置 trait。

**[4. PhantomData——不携带数据的类型](ch04-phantomdata-types-that-carry-no-data.md)** 🔴
生命周期标记、计量单位模式、drop 检查、变型（variance）。

### 第二部分：并发与运行时

**[5. 通道与消息传递](ch05-channels-and-message-passing.md)** 🟢
`std::sync::mpsc`、crossbeam、`select!`、背压、actor 模式。

**[6. 并发、并行与线程](ch06-concurrency-vs-parallelism-vs-threads.md)** 🟡
OS 线程、作用域线程、rayon、Mutex/RwLock/原子操作、Condvar、OnceLock、无锁模式。

**[7. 闭包与高阶函数](ch07-closures-and-higher-order-functions.md)** 🟢
`Fn`/`FnMut`/`FnOnce`、作为参数/返回值的闭包、组合子、高阶 API。

**[8. 函数式 vs 命令式：优雅何时胜出（何时不胜出）](ch08-functional-vs-imperative-when-elegance-wins.md)** 🟡
组合子、迭代器适配器、函数式模式。

**[9. 智能指针与内部可变性](ch09-smart-pointers-and-interior-mutability.md)** 🟡
Box、Rc、Arc、Weak、Cell/RefCell、Cow、Pin、ManuallyDrop。

### 第三部分：系统与生产实践

**[10. 错误处理模式](ch10-error-handling-patterns.md)** 🟢
thiserror vs anyhow、`#[from]`、`.context()`、`?` 运算符、panic。

**[11. 序列化、零拷贝与二进制数据](ch11-serialization-zero-copy-and-binary-data.md)** 🟡
serde 基础、枚举表示、零拷贝反序列化、`repr(C)`、`bytes::Bytes`。

**[12. Unsafe Rust——可控的危险](ch12-unsafe-rust-controlled-danger.md)** 🔴
五大超能力、健全的抽象、FFI、UB 陷阱、arena/slab 分配器。

**[13. 宏——编写代码的代码](ch13-macros-code-that-writes-code.md)** 🟡
`macro_rules!`、何时（不该）使用宏、过程宏、derive 宏、`syn`/`quote`。

**[14. 测试与基准测试模式](ch14-testing-and-benchmarking-patterns.md)** 🟢
单元/集成/文档测试、proptest、criterion、mock 策略。

**[15. Crate 架构与 API 设计](ch15-crate-architecture-and-api-design.md)** 🟡
模块布局、API 设计清单、人体工学参数、feature flag、workspace。

**[16. Async/Await 基础](ch16-asyncawait-essentials.md)** 🔴
Future、Tokio 快速入门、常见陷阱。（如需深入的 async 内容，请参阅我们的 Async Rust 培训。）

### 附录

**[总结与参考卡片](ch18-summary-and-reference-card.md)**
模式决策指南、trait 约束速查表、生命周期省略规则、延伸阅读。

**[综合项目：类型安全的任务调度器](ch19-capstone-project.md)**
将泛型、trait、类型状态、通道、错误处理和测试整合为一个完整系统。

***
