## 快速参考卡

### 模式决策指南

```text
需要基本类型（primitive）的类型安全？
└── 新类型模式（Newtype）（第 3 章）

需要编译期状态强制？
└── 类型状态模式（Type-state）（第 3 章）

需要一个无运行时数据的"标签"？
└── PhantomData（第 4 章）

需要打破 Rc/Arc 引用循环？
└── Weak<T> / sync::Weak<T>（第 8 章）

需要等待条件而不忙等待（busy-looping）？
└── Condvar + Mutex（第 6 章）

需要处理"N 种类型之一"？
├── 已知的封闭集合 → Enum
├── 开放集合，热路径 → 泛型（Generics）
├── 开放集合，冷路径 → dyn Trait
└── 完全未知的类型 → Any + TypeId（第 2 章）

需要跨线程共享状态？
├── 简单计数器/标志 → Atomics
├── 短临界区 → Mutex
├── 读多 → RwLock
├── 惰性一次性初始化 → OnceLock / LazyLock（第 6 章）
└── 复杂状态 → Actor + 通道

需要并行化计算？
├── 集合处理 → rayon::par_iter
├── 后台任务 → thread::spawn
└── 借用本地数据 → thread::scope

需要异步 I/O 或并发网络？
├── 基础 → tokio + async/await（第 15 章）
└── 高级（流、中间件）→ 参见 Async Rust 训练

需要错误处理？
├── 库 → thiserror（#[derive(Error)]）
└── 应用 → anyhow（Result<T>）

需要防止值被移动？
└── Pin<T>（第 8 章）— Future、自引用类型所需
```

### Trait 约束速查表

| 约束 | 含义 |
|-------|---------|
| `T: Clone` | 可被复制 |
| `T: Send` | 可被移动到另一个线程 |
| `T: Sync` | `&T` 可在线程间共享 |
| `T: 'static` | 不包含非 'static 的引用 |
| `T: Sized` | 编译期已知大小（默认） |
| `T: ?Sized` | 大小可能未知（`[T]`、`dyn Trait`） |
| `T: Unpin` | 固定（pin）后移动是安全的 |
| `T: Default` | 有默认值 |
| `T: Into<U>` | 可转换为 `U` |
| `T: AsRef<U>` | 可借用为 `&U` |
| `T: Deref<Target = U>` | 自动解引用为 `&U` |
| `F: Fn(A) -> B` | 可调用，不可变借用状态 |
| `F: FnMut(A) -> B` | 可调用，可能修改状态 |
| `F: FnOnce(A) -> B` | 仅可调用一次，可能消耗状态 |

### 生命周期省略规则

编译器在三种情况下会自动插入生命周期（这样你就不必手动写）：

```rust
// 规则 1：每个引用参数获得自己的生命周期
// fn foo(x: &str, y: &str)  →  fn foo<'a, 'b>(x: &'a str, y: &'b str)

// 规则 2：如果只有一个输入生命周期，它用于所有输出
// fn foo(x: &str) -> &str   →  fn foo<'a>(x: &'a str) -> &'a str

// 规则 3：如果某个参数是 &self 或 &mut self，它的生命周期被使用
// fn foo(&self, x: &str) -> &str  →  fn foo<'a>(&'a self, x: &str) -> &'a str
```

**你必须手动写显式生命周期的情况**：
- 多个输入引用且有一个引用输出（编译器无法猜测哪个输入）
- 持有引用的结构体字段：`struct Ref<'a> { data: &'a str }`
- 当你需要不带借用引用的数据时的 `'static` 约束

### 常见 Derive Trait

```rust
#[derive(
    Debug,          // {:?} 格式化
    Clone,          // .clone()
    Copy,           // 隐式拷贝（仅适用于简单类型）
    PartialEq, Eq,  // == 比较
    PartialOrd, Ord, // < > 比较 + 排序
    Hash,           // HashMap/HashSet 键
    Default,        // Type::default()
)]
struct MyType { /* ... */ }
```

### 模块可见性快速参考

```text
pub           → 对所有人可见
pub(crate)    → 在本 crate 内可见
pub(super)    → 对父模块可见
pub(in path)  → 在特定路径内可见
（无）        → 对当前模块 + 子模块私有
```

### 延伸阅读

| 资源 | 原因 |
|----------|-----|
| [Rust 设计模式](https://rust-unofficial.github.io/patterns/) | 惯用模式与反模式的目录 |
| [Rust API 指南](https://rust-lang.github.io/api-guidelines/) | 精打磨公共 API 的官方清单 |
| [Rust Atomics and Locks](https://marabos.nl/atomics/) | Mara Bos 对并发原语的深入讲解 |
| [The Rustonomicon](https://doc.rust-lang.org/nomicon/) | unsafe Rust 与黑暗角落的官方指南 |
| [Error Handling in Rust](https://blog.burntsushi.net/rust-error-handling/) | Andrew Gallant 的全面指南 |
| [Jon Gjengset — Crust of Rust 系列](https://www.youtube.com/playlist?list=PLqbS7AVVErFiWDOAVrPt7aYmnuuOLYvOa) | 对迭代器、生命周期、通道等的深入讲解 |
| [Effective Rust](https://www.lurklurk.org/effective-rust/) | 改进 Rust 代码的 35 种具体方法 |

***

*Rust 模式与工程实战 完*
