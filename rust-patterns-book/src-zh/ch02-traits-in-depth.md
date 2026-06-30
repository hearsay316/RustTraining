# 2. 深入理解 Trait 🟡

> **你将学到：**
> - 关联类型（associated type）与泛型参数的对比，以及各自的使用时机
> - GAT、泛批实现（blanket impl）、标记 trait（marker trait）以及 trait 对象安全规则
> - vtable 和胖指针（fat pointer）在底层的运作原理
> - 扩展 trait（extension trait）、枚举分派（enum dispatch）和类型化命令模式

## 关联类型 vs 泛型参数

两者都能让 trait 与不同的类型协作，但它们的用途不同：

```rust
// --- 关联类型：每个类型一个实现 ---
trait Iterator {
    type Item; // 每个迭代器只产生一种 Item

    fn next(&mut self) -> Option<Self::Item>;
}

// 一个总是产生 i32 的自定义迭代器——没有其他选择
struct Counter { max: i32, current: i32 }

impl Iterator for Counter {
    type Item = i32; // 每个实现只有一种 Item 类型
    fn next(&mut self) -> Option<i32> {
        if self.current < self.max {
            self.current += 1;
            Some(self.current)
        } else {
            None
        }
    }
}

// --- 泛型参数：每个类型可以有多个实现 ---
trait Convert<T> {
    fn convert(&self) -> T;
}

// 单个类型可以为多种目标类型实现 Convert：
impl Convert<f64> for i32 {
    fn convert(&self) -> f64 { *self as f64 }
}
impl Convert<String> for i32 {
    fn convert(&self) -> String { self.to_string() }
}
```

**何时使用哪种**：

| 使用 | 时机 |
|-----|------|
| **关联类型（associated type）** | 每个实现类型恰好只有一种自然的输出/结果。`Iterator::Item`、`Deref::Target`、`Add::Output` |
| **泛型参数（generic parameter）** | 一个类型可以有意义地为多种不同类型实现该 trait。`From<T>`、`AsRef<T>`、`PartialEq<Rhs>` |

**直觉判断**：如果问"这个迭代器的 `Item` 是什么？"是有意义的，就用关联类型。如果问"它能转换为 `f64` 吗？能转换为 `String` 吗？能转换为 `bool` 吗？"是有意义的，就用泛型参数。

```rust
// 真实案例：std::ops::Add
trait Add<Rhs = Self> {
    type Output; // 关联类型——加法只有一种结果类型
    fn add(self, rhs: Rhs) -> Self::Output;
}

// Rhs 是泛型参数——你可以将不同类型加到 Meters 上：
struct Meters(f64);
struct Centimeters(f64);

impl Add<Meters> for Meters {
    type Output = Meters;
    fn add(self, rhs: Meters) -> Meters { Meters(self.0 + rhs.0) }
}
impl Add<Centimeters> for Meters {
    type Output = Meters;
    fn add(self, rhs: Centimeters) -> Meters { Meters(self.0 + rhs.0 / 100.0) }
}
```

### 泛型关联类型（GAT）

从 Rust 1.65 开始，关联类型可以拥有自己的泛型参数。
这使得**借贷迭代器（lending iterator）**成为可能——这种迭代器返回的引用与
迭代器本身绑定，而不是与底层集合绑定：

```rust
// 没有 GATs——无法表达借贷迭代器：
// trait LendingIterator {
//     type Item<'a>;  // ← 在 1.65 之前被拒绝
// }

// 使用 GATs（Rust 1.65+）：
trait LendingIterator {
    type Item<'a> where Self: 'a;

    fn next(&mut self) -> Option<Self::Item<'_>>;
}

// 示例：一个产生重叠窗口的迭代器
struct WindowIter<'data> {
    data: &'data [u8],
    pos: usize,
    window_size: usize,
}

impl<'data> LendingIterator for WindowIter<'data> {
    type Item<'a> = &'a [u8] where Self: 'a;

    fn next(&mut self) -> Option<&[u8]> {
        if self.pos + self.window_size <= self.data.len() {
            let window = &self.data[self.pos..self.pos + self.window_size];
            self.pos += 1;
            Some(window)
        } else {
            None
        }
    }
}
```

> **何时需要 GAT**：借贷迭代器、流式解析器，或者任何关联类型的生命周期
> 依赖于 `&self` 借用的 trait。对于大多数代码，普通的关联类型就足够了。

### 父 trait 与 trait 层次结构

Trait 可以要求其他 trait 作为前提条件，从而形成层次结构：

```mermaid
graph BT
    Display["Display"]
    Debug["Debug"]
    Error["Error"]
    Clone["Clone"]
    Copy["Copy"]
    PartialEq["PartialEq"]
    Eq["Eq"]
    PartialOrd["PartialOrd"]
    Ord["Ord"]

    Error --> Display
    Error --> Debug
    Copy --> Clone
    Eq --> PartialEq
    Ord --> Eq
    Ord --> PartialOrd
    PartialOrd --> PartialEq

    style Display fill:#e8f4f8,stroke:#2980b9,color:#000
    style Debug fill:#e8f4f8,stroke:#2980b9,color:#000
    style Error fill:#fdebd0,stroke:#e67e22,color:#000
    style Clone fill:#d4efdf,stroke:#27ae60,color:#000
    style Copy fill:#d4efdf,stroke:#27ae60,color:#000
    style PartialEq fill:#fef9e7,stroke:#f1c40f,color:#000
    style Eq fill:#fef9e7,stroke:#f1c40f,color:#000
    style PartialOrd fill:#fef9e7,stroke:#f1c40f,color:#000
    style Ord fill:#fef9e7,stroke:#f1c40f,color:#000
```

> 箭头从子 trait 指向父 trait：实现 `Error` 需要 `Display` + `Debug`。

一个 trait 可以要求实现者同时实现其他 trait：

```rust
use std::fmt;

// Display 是 Error 的父 trait
trait Error: fmt::Display + fmt::Debug {
    fn source(&self) -> Option<&(dyn Error + 'static)> { None }
}
// 实现 Error 的任何类型都必须同时实现 Display 和 Debug

// 构建你自己的层次结构：
trait Identifiable {
    fn id(&self) -> u64;
}

trait Timestamped {
    fn created_at(&self) -> chrono::DateTime<chrono::Utc>;
}

// Entity 要求两者都有：
trait Entity: Identifiable + Timestamped {
    fn is_active(&self) -> bool;
}

// 实现 Entity 会强制你实现全部三个 trait：
struct User { id: u64, name: String, created: chrono::DateTime<chrono::Utc> }

impl Identifiable for User {
    fn id(&self) -> u64 { self.id }
}
impl Timestamped for User {
    fn created_at(&self) -> chrono::DateTime<chrono::Utc> { self.created }
}
impl Entity for User {
    fn is_active(&self) -> bool { true }
}
```

### 泛批实现（Blanket Implementations）

为满足某个约束的所有类型实现一个 trait：

```rust
// 标准库的做法：任何实现了 Display 的类型自动获得 ToString
impl<T: fmt::Display> ToString for T {
    fn to_string(&self) -> String {
        format!("{self}")
    }
}
// 现在 i32、&str、你的自定义类型——任何有 Display 的类型——都免费获得 to_string()。

// 你自己的泛批实现：
trait Loggable {
    fn log(&self);
}

// 每个 Debug 类型都自动成为 Loggable：
impl<T: std::fmt::Debug> Loggable for T {
    fn log(&self) {
        eprintln!("[LOG] {self:?}");
    }
}

// 现在任何 Debug 类型都有 .log()：
// 42.log();              // [LOG] 42
// "hello".log();         // [LOG] "hello"
// vec![1, 2, 3].log();   // [LOG] [1, 2, 3]
```

> **注意**：泛批实现功能强大但不可逆——你无法为已被泛批实现覆盖的类型
> 添加更具体的实现（孤儿规则 + 一致性规则）。请谨慎设计。

### 标记 trait（Marker Traits）

没有方法的 trait——它们只是将某个类型标记为具有某种属性：

```rust
// 标准库的标记 trait：
// Send    — 可以安全地在线程间转移
// Sync    — 可以安全地在线程间共享（&T）
// Unpin   — 固定后可以安全移动
// Sized   — 编译期已知大小
// Copy    — 可以用 memcpy 复制

// 你自己的标记 trait：
/// 标记：这个传感器已经过出厂校准
trait Calibrated {}

struct RawSensor { reading: f64 }
struct CalibratedSensor { reading: f64 }

impl Calibrated for CalibratedSensor {}

// 只有校准过的传感器才能用于生产环境：
fn record_measurement<S: Calibrated>(sensor: &S) {
    // ...
}
// record_measurement(&RawSensor { reading: 0.0 }); // ❌ 编译错误
// record_measurement(&CalibratedSensor { reading: 0.0 }); // ✅
```

这与第 3 章中的**类型状态（type-state）模式**直接相关。

### Trait 对象安全规则

并非所有 trait 都能用作 `dyn Trait`。一个 trait 只有满足以下条件才是**对象安全（object-safe）**的：

1. **trait 本身没有 `Self: Sized` 约束**
2. **方法没有泛型类型参数**
3. **返回位置不使用 `Self`**（除非通过间接方式如 `Box<Self>`）
4. **没有关联函数**（方法必须有 `&self`、`&mut self` 或 `self`）

```rust
// ✅ 对象安全——可以用作 dyn Drawable
trait Drawable {
    fn draw(&self);
    fn bounding_box(&self) -> (f64, f64, f64, f64);
}

let shapes: Vec<Box<dyn Drawable>> = vec![/* ... */]; // ✅ 可用

// ❌ 不对象安全——在返回位置使用了 Self
trait Cloneable {
    fn clone_self(&self) -> Self;
    //                       ^^^^ 运行时无法知道具体大小
}
// let items: Vec<Box<dyn Cloneable>> = ...; // ❌ 编译错误

// ❌ 不对象安全——泛型方法
trait Converter {
    fn convert<T>(&self) -> T;
    //        ^^^ vtable 无法包含无限的单态化版本
}

// ❌ 不对象安全——关联函数（没有 self）
trait Factory {
    fn create() -> Self;
    // 没有 &self——如何通过 trait 对象调用这个？
}
```

**变通方法**：

```rust
// 添加 `where Self: Sized` 将方法排除在 vtable 之外：
trait MyTrait {
    fn regular_method(&self); // 包含在 vtable 中

    fn generic_method<T>(&self) -> T
    where
        Self: Sized; // 从 vtable 中排除——不能通过 dyn MyTrait 调用
}

// 现在 dyn MyTrait 是有效的，但 generic_method 只能在
// 已知具体类型时调用。
```

> **经验法则**：如果你计划使用 `dyn Trait`，请保持方法简单——
> 不要使用泛型、返回类型中不要出现 `Self`、不要用 `Sized` 约束。拿不准时，
> 试试 `let _: Box<dyn YourTrait>;`，让编译器告诉你结果。

### Trait 对象的底层原理——vtable 与胖指针

`&dyn Trait`（或 `Box<dyn Trait>`）是一个**胖指针（fat pointer）**——由两个机器字组成：

```text
┌──────────────────────────────────────────────────┐
│  &dyn Drawable (on 64-bit: 16 bytes total)       │
├──────────────┬───────────────────────────────────┤
│  data_ptr    │  vtable_ptr                       │
│  (8 bytes)   │  (8 bytes)                        │
│  ↓           │  ↓                                │
│  ┌─────────┐ │  ┌──────────────────────────────┐ │
│  │ Circle  │ │  │ vtable for <Circle as        │ │
│  │ {       │ │  │           Drawable>          │ │
│  │  r: 5.0 │ │  │                              │ │
│  │ }       │ │  │  drop_in_place: 0x7f...a0    │ │
│  └─────────┘ │  │  size:           8           │ │
│              │  │  align:          8           │ │
│              │  │  draw:          0x7f...b4    │ │
│              │  │  bounding_box:  0x7f...c8    │ │
│              │  └──────────────────────────────┘ │
└──────────────┴───────────────────────────────────┘
```

**vtable 调用的工作原理**（例如 `shape.draw()`）：

1. 从胖指针（第二个字）中加载 `vtable_ptr`
2. 在 vtable 中索引查找 `draw` 函数指针
3. 调用它，将 `data_ptr` 作为 `self` 参数传入

这与 C++ 虚函数分派的成本类似（每次调用一次指针间接寻址），
但 Rust 将 vtable 指针存储在胖指针中，而不是对象内部——因此栈上的
普通 `Circle` 根本不携带 vtable 指针。

```rust
trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}

struct Circle { radius: f64 }

impl Drawable for Circle {
    fn draw(&self) { println!("Drawing circle r={}", self.radius); }
    fn area(&self) -> f64 { std::f64::consts::PI * self.radius * self.radius }
}

struct Square { side: f64 }

impl Drawable for Square {
    fn draw(&self) { println!("Drawing square s={}", self.side); }
    fn area(&self) -> f64 { self.side * self.side }
}

fn main() {
    let shapes: Vec<Box<dyn Drawable>> = vec![
        Box::new(Circle { radius: 5.0 }),
        Box::new(Square { side: 3.0 }),
    ];

    // 每个元素都是一个胖指针：(data_ptr, vtable_ptr)
    // Circle 和 Square 的 vtable 是不同的
    for shape in &shapes {
        shape.draw();  // vtable 分派 → Circle::draw 或 Square::draw
        println!("  area = {:.2}", shape.area());
    }

    // 大小比较：
    println!("size_of::<&Circle>()        = {}", size_of::<&Circle>());
    // → 8 字节（一个指针——编译器知道类型）
    println!("size_of::<&dyn Drawable>()  = {}", size_of::<&dyn Drawable>());
    // → 16 字节（data_ptr + vtable_ptr）
}
```

**性能成本模型**：

| 方面 | 静态分派（`impl Trait` / 泛型） | 动态分派（`dyn Trait`） |
|--------|------------------------------------------|-------------------------------|
| 调用开销 | 零——由 LLVM 内联 | 每次调用一次指针间接寻址 |
| 内联 | ✅ 编译器可内联 | ❌ 不透明的函数指针 |
| 二进制大小 | 更大（每种类型一份副本） | 更小（一个共享函数） |
| 指针大小 | 瘦指针（1 个字） | 胖指针（2 个字） |
| 异构集合 | ❌ | ✅ `Vec<Box<dyn Trait>>` |

> **vtable 开销何时重要**：在紧凑循环中数百万次调用 trait 方法时，
> 间接寻址和无法内联可能造成显著影响（慢 2-10 倍）。对于冷路径、
> 配置或插件架构，`dyn Trait` 的灵活性值得这点小小的开销。

### 高阶 trait 约束（HRTBs）

有时你需要一个能处理*任意*生命周期引用（而非某个特定生命周期）的函数。这就是 `for<'a>` 语法出现的地方：

```rust
// 问题：这个函数需要一个能处理
// 任意生命周期的引用的闭包，而不是某个特定生命周期。

// ❌ 这太严格了——'a 由调用者固定：
// fn apply<'a, F: Fn(&'a str) -> &'a str>(f: F, data: &'a str) -> &'a str

// ✅ HRTB：F 必须适用于所有可能的生命周期：
fn apply<F>(f: F, data: &str) -> &str
where
    F: for<'a> Fn(&'a str) -> &'a str,
{
    f(data)
}

fn main() {
    let result = apply(|s| s.trim(), "  hello  ");
    println!("{result}"); // "hello"
}
```

**你会在以下场景遇到 HRTB**：
- `Fn(&T) -> &U` trait——在大多数情况下编译器会自动推断 `for<'a>`
- 必须跨不同借用工作的自定义 trait 实现
- 使用 `serde` 反序列化：`for<'de> Deserialize<'de>`

```rust,ignore
// serde 的 DeserializeOwned 定义为：
// trait DeserializeOwned: for<'de> Deserialize<'de> {}
// 含义："可以从任意生命周期的数据中反序列化"
// （即结果不从输入借用）

use serde::de::DeserializeOwned;

fn parse_json<T: DeserializeOwned>(input: &str) -> T {
    serde_json::from_str(input).unwrap()
}
```

> **实用建议**：你很少会自己编写 `for<'a>`。它主要出现在闭包参数的
> trait 约束中，编译器会隐式处理。但在错误信息中认出它
> （"expected a `for<'a> Fn(&'a ...)` bound"）能帮助你理解编译器在要求什么。

### `impl Trait`——参数位置 vs 返回位置

`impl Trait` 出现在两个位置时具有**不同的语义**：

```rust
// --- 参数位置的 impl Trait (APIT) ---
// "调用者选择类型"——泛型参数的语法糖
fn print_all(items: impl Iterator<Item = i32>) {
    for item in items { println!("{item}"); }
}
// 等价于：
fn print_all_verbose<I: Iterator<Item = i32>>(items: I) {
    for item in items { println!("{item}"); }
}
// 调用者决定：print_all(vec![1,2,3].into_iter())
//             print_all(0..10)

// --- 返回位置的 impl Trait (RPIT) ---
// "被调用者选择类型"——函数挑选一个具体类型
fn evens(limit: i32) -> impl Iterator<Item = i32> {
    (0..limit).filter(|x| x % 2 == 0)
    // 具体类型是 Filter<Range<i32>, Closure>
    // 但调用者只看到"某个 Iterator<Item = i32>"
}
```

**关键区别**：

| | APIT（`fn foo(x: impl T)`） | RPIT（`fn foo() -> impl T`） |
|---|---|---|
| 谁选择类型？ | 调用者 | 被调用者（函数体） |
| 是否单态化？ | 是——每种类型一份副本 | 是——一个具体类型 |
| 能否用 turbofish？ | 不能（`foo::<X>()` 不允许） | 不适用 |
| 等价于 | `fn foo<X: T>(x: X)` | 存在类型 |

#### trait 定义中的 RPIT（RPITIT）

从 Rust 1.75 开始，你可以直接在 trait 定义中使用 `-> impl Trait`：

```rust
trait Container {
    fn items(&self) -> impl Iterator<Item = &str>;
    //                 ^^^^ Each implementor returns its own concrete type
}

struct CsvRow {
    fields: Vec<String>,
}

impl Container for CsvRow {
    fn items(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(String::as_str)
    }
}

struct FixedFields;

impl Container for FixedFields {
    fn items(&self) -> impl Iterator<Item = &str> {
        ["host", "port", "timeout"].into_iter()
    }
}
```

> **在 Rust 1.75 之前**，你必须在 trait 中使用 `Box<dyn Iterator>` 或
> 关联类型来实现这一点。RPITIT 去除了内存分配。

#### `impl Trait` vs `dyn Trait`——决策指南

```text
Do you know the concrete type at compile time?
├── YES → Use impl Trait or generics (zero cost, inlinable)
└── NO  → Do you need a heterogeneous collection?
     ├── YES → Use dyn Trait (Box<dyn T>, &dyn T)
     └── NO  → Do you need the SAME trait object across an API boundary?
          ├── YES → Use dyn Trait
          └── NO  → Use generics / impl Trait
```

| 特性 | `impl Trait` | `dyn Trait` |
|---------|-------------|------------|
| 分派 | 静态（单态化） | 动态（vtable） |
| 性能 | 最佳——可内联 | 每次调用一次间接寻址 |
| 异构集合 | ❌ | ✅ |
| 每种类型的二进制大小 | 每种一份副本 | 共享代码 |
| trait 必须对象安全？ | 否 | 是 |
| 可用于 trait 定义 | ✅（Rust 1.75+） | 始终可以 |

***

## 使用 `Any` 和 `TypeId` 进行类型擦除

有时你需要存储*未知*类型的值并在之后对其进行向下转型（downcast）——这是一个
从 C 的 `void*` 或 C# 的 `object` 中熟悉的模式。Rust 通过 `std::any::Any` 提供了这一能力：

```rust
use std::any::Any;

// 存储异构值：
fn log_value(value: &dyn Any) {
    if let Some(s) = value.downcast_ref::<String>() {
        println!("String: {s}");
    } else if let Some(n) = value.downcast_ref::<i32>() {
        println!("i32: {n}");
    } else {
        // TypeId 让你在运行时检查类型：
        println!("Unknown type: {:?}", value.type_id());
    }
}

// 适用于插件系统、事件总线或 ECS 风格的架构：
struct AnyMap(std::collections::HashMap<std::any::TypeId, Box<dyn Any + Send>>);

impl AnyMap {
    fn new() -> Self { AnyMap(std::collections::HashMap::new()) }

    fn insert<T: Any + Send + 'static>(&mut self, value: T) {
        self.0.insert(std::any::TypeId::of::<T>(), Box::new(value));
    }

    fn get<T: Any + Send + 'static>(&self) -> Option<&T> {
        self.0.get(&std::any::TypeId::of::<T>())?
            .downcast_ref()
    }
}

fn main() {
    let mut map = AnyMap::new();
    map.insert(42_i32);
    map.insert(String::from("hello"));

    assert_eq!(map.get::<i32>(), Some(&42));
    assert_eq!(map.get::<String>().map(|s| s.as_str()), Some("hello"));
    assert_eq!(map.get::<f64>(), None); // Never inserted
}
```

> **何时使用 `Any`**：插件/扩展系统、类型索引映射（`typemap`）、
> 错误向下转型（`anyhow::Error::downcast_ref`）。当类型集合在编译时已知时，
> 优先使用泛型或 trait 对象——`Any` 是最后的手段，
> 它以牺牲编译时安全性来换取灵活性。

***

## 扩展 trait——为你不拥有的类型添加方法

Rust 的孤儿规则（orphan rule）阻止你为外部类型实现外部 trait。
扩展 trait 是标准的变通方法：在你的 crate 中定义一个**新 trait**，其方法
通过泛批实现自动应用于满足约束的任何类型。调用者导入该 trait 后，
新方法就会出现在现有类型上。

这种模式在 Rust 生态系统中无处不在：`itertools::Itertools`、`futures::StreamExt`、
`tokio::io::AsyncReadExt`、`tower::ServiceExt`。

### 问题所在

```rust
// We want to add a .mean() method to all iterators that yield f64.
// But Iterator is defined in std and f64 is a primitive — orphan rule prevents:
//
// impl<I: Iterator<Item = f64>> I {   // ❌ 不能为外部类型添加固有方法
//     fn mean(self) -> f64 { ... }
// }
```

### 解决方案：扩展 trait

```rust
/// Extension methods for iterators over numeric values.
pub trait IteratorExt: Iterator {
    /// Computes the arithmetic mean. Returns `None` for empty iterators.
    fn mean(self) -> Option<f64>
    where
        Self: Sized,
        Self::Item: Into<f64>;
}

// 泛批实现——自动应用于所有迭代器
impl<I: Iterator> IteratorExt for I {
    fn mean(self) -> Option<f64>
    where
        Self: Sized,
        Self::Item: Into<f64>,
    {
        let mut sum: f64 = 0.0;
        let mut count: u64 = 0;
        for item in self {
            sum += item.into();
            count += 1;
        }
        if count == 0 { None } else { Some(sum / count as f64) }
    }
}

// Usage — just import the trait:
use crate::IteratorExt;  // 导入一次，方法就出现在所有迭代器上

fn analyze_temperatures(readings: &[f64]) -> Option<f64> {
    readings.iter().copied().mean()  // .mean() 现在可用了！
}

fn analyze_sensor_data(data: &[i32]) -> Option<f64> {
    data.iter().copied().mean()  // Works on i32 too (i32: Into<f64>)
}
```

### 真实案例：诊断结果扩展

```rust
use std::collections::HashMap;

struct DiagResult {
    component: String,
    passed: bool,
    message: String,
}

/// Extension trait for Vec<DiagResult> — adds domain-specific analysis methods.
pub trait DiagResultsExt {
    fn passed_count(&self) -> usize;
    fn failed_count(&self) -> usize;
    fn overall_pass(&self) -> bool;
    fn failures_by_component(&self) -> HashMap<String, Vec<&DiagResult>>;
}

impl DiagResultsExt for Vec<DiagResult> {
    fn passed_count(&self) -> usize {
        self.iter().filter(|r| r.passed).count()
    }

    fn failed_count(&self) -> usize {
        self.iter().filter(|r| !r.passed).count()
    }

    fn overall_pass(&self) -> bool {
        self.iter().all(|r| r.passed)
    }

    fn failures_by_component(&self) -> HashMap<String, Vec<&DiagResult>> {
        let mut map = HashMap::new();
        for r in self.iter().filter(|r| !r.passed) {
            map.entry(r.component.clone()).or_default().push(r);
        }
        map
    }
}

// Now any Vec<DiagResult> has these methods:
fn report(results: Vec<DiagResult>) {
    if !results.overall_pass() {
        let failures = results.failures_by_component();
        for (component, fails) in &failures {
            eprintln!("{component}: {} failures", fails.len());
        }
    }
}
```

### 命名约定

Rust 生态系统使用一致的 `Ext` 后缀：

| crate | 扩展 trait | 扩展的对象 |
|-------|----------------|---------|
| `itertools` | `Itertools` | `Iterator` |
| `futures` | `StreamExt`、`FutureExt` | `Stream`、`Future` |
| `tokio` | `AsyncReadExt`、`AsyncWriteExt` | `AsyncRead`、`AsyncWrite` |
| `tower` | `ServiceExt` | `Service` |
| `bytes` | `BufMut`（部分） | `&mut [u8]` |
| 你的 crate | `DiagResultsExt` | `Vec<DiagResult>` |

### 何时使用

| 场景 | 是否使用扩展 trait？ |
|-----------|:---:|
| 为外部类型添加便捷方法 | ✅ |
| 将领域特定逻辑组织在泛型集合上 | ✅ |
| 方法需要访问私有字段 | ❌（使用包装器/newtype） |
| 方法在逻辑上属于你控制的新类型 | ❌（直接添加到你的类型上） |
| 你希望方法无需导入即可使用 | ❌（仅限固有方法） |

***

## 枚举分派——无需 `dyn` 的静态多态

当你有一个实现某 trait 的**封闭**类型集合时，可以用一个枚举来替代 `dyn Trait`，
其变体持有具体类型。这消除了 vtable 间接寻址和堆分配，
同时保留了相同的调用方接口。

### `dyn Trait` 的问题

```rust
trait Sensor {
    fn read(&self) -> f64;
    fn name(&self) -> &str;
}

struct Gps { lat: f64, lon: f64 }
struct Thermometer { temp_c: f64 }
struct Accelerometer { g_force: f64 }

impl Sensor for Gps {
    fn read(&self) -> f64 { self.lat }
    fn name(&self) -> &str { "GPS" }
}
impl Sensor for Thermometer {
    fn read(&self) -> f64 { self.temp_c }
    fn name(&self) -> &str { "Thermometer" }
}
impl Sensor for Accelerometer {
    fn read(&self) -> f64 { self.g_force }
    fn name(&self) -> &str { "Accelerometer" }
}

// Heterogeneous collection with dyn — works, but has costs:
fn read_all_dyn(sensors: &[Box<dyn Sensor>]) -> Vec<f64> {
    sensors.iter().map(|s| s.read()).collect()
    // 每次 .read() 都经过 vtable 间接寻址
    // 每个 Box 都在堆上分配
}
```

### 枚举分派解决方案

```rust
// Replace the trait object with an enum:
enum AnySensor {  // 用枚举替代 trait 对象
    Gps(Gps),
    Thermometer(Thermometer),
    Accelerometer(Accelerometer),
}

impl AnySensor {
    fn read(&self) -> f64 {
        match self {
            AnySensor::Gps(s) => s.read(),
            AnySensor::Thermometer(s) => s.read(),
            AnySensor::Accelerometer(s) => s.read(),
        }
    }

    fn name(&self) -> &str {
        match self {
            AnySensor::Gps(s) => s.name(),
            AnySensor::Thermometer(s) => s.name(),
            AnySensor::Accelerometer(s) => s.name(),
        }
    }
}

// Now: no heap allocation, no vtable, stored inline
fn read_all(sensors: &[AnySensor]) -> Vec<f64> {
    sensors.iter().map(|s| s.read()).collect()
    // 现在：无堆分配，无 vtable，内联存储
    // 每次 .read() 都是一个 match 分支——编译器可以全部内联
}

fn main() {
    let sensors = vec![
        AnySensor::Gps(Gps { lat: 47.6, lon: -122.3 }),
        AnySensor::Thermometer(Thermometer { temp_c: 72.5 }),
        AnySensor::Accelerometer(Accelerometer { g_force: 1.02 }),
    ];

    for sensor in &sensors {
        println!("{}: {:.2}", sensor.name(), sensor.read());
    }
}
```

### 在枚举上实现 trait

为了互操作性，你可以在枚举本身上实现原始 trait：

```rust
impl Sensor for AnySensor {
    fn read(&self) -> f64 {
        match self {
            AnySensor::Gps(s) => s.read(),
            AnySensor::Thermometer(s) => s.read(),
            AnySensor::Accelerometer(s) => s.read(),
        }
    }

    fn name(&self) -> &str {
        match self {
            AnySensor::Gps(s) => s.name(),
            AnySensor::Thermometer(s) => s.name(),
            AnySensor::Accelerometer(s) => s.name(),
        }
    }
}

// Now AnySensor works anywhere a Sensor is expected via generics:
fn report<S: Sensor>(s: &S) {  // 现在 AnySensor 可以在任何期望 Sensor 的地方通过泛型使用
    println!("{}: {:.2}", s.name(), s.read());
}
```

### 用宏减少样板代码

match 分支的委派是重复的。宏可以消除它：

```rust
macro_rules! dispatch_sensor {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            AnySensor::Gps(s) => s.$method($($arg),*),
            AnySensor::Thermometer(s) => s.$method($($arg),*),
            AnySensor::Accelerometer(s) => s.$method($($arg),*),
        }
    };
}

impl Sensor for AnySensor {
    fn read(&self) -> f64     { dispatch_sensor!(self, read) }
    fn name(&self) -> &str    { dispatch_sensor!(self, name) }
}
```

对于更大的项目，`enum_dispatch` crate 可以完全自动化这个过程：

```rust
use enum_dispatch::enum_dispatch;

#[enum_dispatch]
trait Sensor {
    fn read(&self) -> f64;
    fn name(&self) -> &str;
}

#[enum_dispatch(Sensor)]
enum AnySensor {
    Gps,
    Thermometer,
    Accelerometer,
}
// All delegation code is generated automatically.
// 所有委派代码都自动生成。
```

### `dyn Trait` vs 枚举分派——决策指南

```text
Is the set of types closed (known at compile time)?
├── YES → Prefer enum dispatch (faster, no heap allocation)
│         ├── Few variants (< ~20)?     → Manual enum
│         └── Many variants or growing? → enum_dispatch crate
└── NO  → Must use dyn Trait (plugins, user-provided types)
```

| 属性 | `dyn Trait` | 枚举分派 |
|----------|:-----------:|:-------------:|
| 分派开销 | vtable 间接寻址（约 2ns） | 分支预测（约 0.3ns） |
| 堆分配 | 通常需要（Box） | 无（内联存储） |
| 缓存友好 | 否（指针追逐） | 是（连续存储） |
| 对新类型开放 | ✅（任何人都能实现） | ❌（封闭集合） |
| 代码大小 | 共享 | 每个变体一份副本 |
| trait 必须对象安全 | 是 | 否 |
| 添加变体 | 无需改代码 | 更新枚举 + match 分支 |

### 何时使用枚举分派

| 场景 | 推荐 |
|----------|---------------|
| 诊断测试类型（CPU、GPU、网卡、内存……） | ✅ 枚举分派——封闭集合，编译时已知 |
| 总线协议（SPI、I2C、UART……） | ✅ 枚举分派或 config trait |
| 插件系统（用户在运行时加载 .so） | ❌ 使用 `dyn Trait` |
| 2-3 个变体 | ✅ 手动枚举分派 |
| 10+ 个变体且方法众多 | ✅ `enum_dispatch` crate |
| 性能关键的内部循环 | ✅ 枚举分派（消除 vtable） |

***

## 能力混入——关联类型实现零开销组合

Ruby 开发者用**混入（mixin）**来组合行为——`include SomeModule` 将方法注入到
类中。Rust 的 trait 配合**关联类型 + 默认方法 + 泛批实现**能产生相同的效果，
不同之处在于：

* 一切都在**编译时**解析——没有 method-missing 的意外
* 每个关联类型都是一个**旋钮**，可以改变默认方法产生的内容
* 编译器对每种组合进行**单态化**——零 vtable 开销

### 问题：横切总线依赖

硬件诊断例程共享一些通用操作——读取 IPMI 传感器、切换 GPIO 电源轨、
通过 SPI 采样温度——但不同的诊断需要不同的组合。Rust 中不存在继承层次结构。
将每个总线句柄作为函数参数传递会创建笨重的签名。我们需要一种方式来按需
**混入**总线能力。

### 第 1 步——定义"原料"trait

每个原料通过一个关联类型提供一种硬件能力：

```rust
use std::io;

// ── Bus abstractions (traits the hardware team provides) ──────────
pub trait SpiBus {
    fn spi_transfer(&self, tx: &[u8], rx: &mut [u8]) -> io::Result<()>;
}

pub trait I2cBus {
    fn i2c_read(&self, addr: u8, reg: u8, buf: &mut [u8]) -> io::Result<()>;
    fn i2c_write(&self, addr: u8, reg: u8, data: &[u8]) -> io::Result<()>;
}

pub trait GpioPin {
    fn set_high(&self) -> io::Result<()>;
    fn set_low(&self) -> io::Result<()>;
    fn read_level(&self) -> io::Result<bool>;
}

pub trait IpmiBmc {
    fn raw_command(&self, net_fn: u8, cmd: u8, data: &[u8]) -> io::Result<Vec<u8>>;
    fn read_sensor(&self, sensor_id: u8) -> io::Result<f64>;
}

// ── Ingredient traits — one per bus, carries an associated type ───
pub trait HasSpi {
    type Spi: SpiBus;
    fn spi(&self) -> &Self::Spi;
}

pub trait HasI2c {
    type I2c: I2cBus;
    fn i2c(&self) -> &Self::I2c;
}

pub trait HasGpio {
    type Gpio: GpioPin;
    fn gpio(&self) -> &Self::Gpio;
}

pub trait HasIpmi {
    type Ipmi: IpmiBmc;
    fn ipmi(&self) -> &Self::Ipmi;
}
```

每个原料都是小巧、通用且可独立测试的。

### 第 2 步——定义"混入"trait

混入 trait 将其所需的原料声明为父 trait，然后通过**默认实现**提供
其所有方法——实现者免费获得这些方法：

```rust
/// Mixin: fan diagnostics — needs I2C (tachometer) + GPIO (PWM enable)
pub trait FanDiagMixin: HasI2c + HasGpio {
    /// Read fan RPM from the tachometer IC over I2C.
    fn read_fan_rpm(&self, fan_id: u8) -> io::Result<u32> {
        let mut buf = [0u8; 2];
        self.i2c().i2c_read(0x48 + fan_id, 0x00, &mut buf)?;
        Ok(u16::from_be_bytes(buf) as u32 * 60) // tach counts → RPM
    }

    /// Enable or disable the fan PWM output via GPIO.
    fn set_fan_pwm(&self, enable: bool) -> io::Result<()> {
        if enable { self.gpio().set_high() }
        else      { self.gpio().set_low() }
    }

    /// Full fan health check — read RPM + verify within threshold.
    fn check_fan_health(&self, fan_id: u8, min_rpm: u32) -> io::Result<bool> {
        let rpm = self.read_fan_rpm(fan_id)?;
        Ok(rpm >= min_rpm)
    }
}

/// Mixin: temperature monitoring — needs SPI (thermocouple ADC) + IPMI (BMC sensors)
pub trait TempMonitorMixin: HasSpi + HasIpmi {
    /// Read a thermocouple via the SPI ADC (e.g. MAX31855).
    fn read_thermocouple(&self) -> io::Result<f64> {
        let mut rx = [0u8; 4];
        self.spi().spi_transfer(&[0x00; 4], &mut rx)?;
        let raw = i32::from_be_bytes(rx) >> 18; // 14-bit signed
        Ok(raw as f64 * 0.25)
    }

    /// Read a BMC-managed temperature sensor via IPMI.
    fn read_bmc_temp(&self, sensor_id: u8) -> io::Result<f64> {
        self.ipmi().read_sensor(sensor_id)
    }

    /// Cross-validate: thermocouple vs BMC must agree within delta.
    fn validate_temps(&self, sensor_id: u8, max_delta: f64) -> io::Result<bool> {
        let tc = self.read_thermocouple()?;
        let bmc = self.read_bmc_temp(sensor_id)?;
        Ok((tc - bmc).abs() <= max_delta)
    }
}

/// Mixin: power sequencing — needs GPIO (rail enable) + IPMI (event logging)
pub trait PowerSeqMixin: HasGpio + HasIpmi {
    /// Assert the power-good GPIO and verify via IPMI sensor.
    fn enable_power_rail(&self, sensor_id: u8) -> io::Result<bool> {
        self.gpio().set_high()?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        let voltage = self.ipmi().read_sensor(sensor_id)?;
        Ok(voltage > 0.8) // above 80% nominal = good
    }

    /// De-assert power and log shutdown via IPMI OEM command.
    fn disable_power_rail(&self) -> io::Result<()> {
        self.gpio().set_low()?;
        // Log OEM "power rail disabled" event to BMC
        self.ipmi().raw_command(0x2E, 0x01, &[0x00, 0x01])?;
        Ok(())
    }
}
```

### 第 3 步——泛批实现使其成为真正的"混入"

神奇的一行——提供原料，即可获得方法：

```rust
impl<T: HasI2c + HasGpio>  FanDiagMixin    for T {}
impl<T: HasSpi  + HasIpmi>  TempMonitorMixin for T {}
impl<T: HasGpio + HasIpmi>  PowerSeqMixin   for T {}
```

任何实现了正确原料 trait 的结构体**自动**获得所有混入方法——
没有样板代码、没有转发、没有继承。

### 第 4 步——组装生产环境

```rust
// ── Concrete bus implementations (Linux platform) ────────────────
struct LinuxSpi  { dev: String }
struct LinuxI2c  { dev: String }
struct SysfsGpio { pin: u32 }
struct IpmiTool  { timeout_secs: u32 }

impl SpiBus for LinuxSpi {
    fn spi_transfer(&self, _tx: &[u8], _rx: &mut [u8]) -> io::Result<()> {
        // spidev ioctl — omitted for brevity
        Ok(())
    }
}
impl I2cBus for LinuxI2c {
    fn i2c_read(&self, _addr: u8, _reg: u8, _buf: &mut [u8]) -> io::Result<()> {
        // i2c-dev ioctl — omitted for brevity
        Ok(())
    }
    fn i2c_write(&self, _addr: u8, _reg: u8, _data: &[u8]) -> io::Result<()> { Ok(()) }
}
impl GpioPin for SysfsGpio {
    fn set_high(&self) -> io::Result<()>  { /* /sys/class/gpio */ Ok(()) }
    fn set_low(&self) -> io::Result<()>   { Ok(()) }
    fn read_level(&self) -> io::Result<bool> { Ok(true) }
}
impl IpmiBmc for IpmiTool {
    fn raw_command(&self, _nf: u8, _cmd: u8, _data: &[u8]) -> io::Result<Vec<u8>> {
        // shells out to ipmitool — omitted for brevity
        Ok(vec![])
    }
    fn read_sensor(&self, _id: u8) -> io::Result<f64> { Ok(25.0) }
}

// ── Production platform — all four buses ─────────────────────────
struct DiagPlatform {
    spi:  LinuxSpi,
    i2c:  LinuxI2c,
    gpio: SysfsGpio,
    ipmi: IpmiTool,
}

impl HasSpi  for DiagPlatform { type Spi  = LinuxSpi;  fn spi(&self)  -> &LinuxSpi  { &self.spi  } }
impl HasI2c  for DiagPlatform { type I2c  = LinuxI2c;  fn i2c(&self)  -> &LinuxI2c  { &self.i2c  } }
impl HasGpio for DiagPlatform { type Gpio = SysfsGpio; fn gpio(&self) -> &SysfsGpio { &self.gpio } }
impl HasIpmi for DiagPlatform { type Ipmi = IpmiTool;  fn ipmi(&self) -> &IpmiTool  { &self.ipmi } }

// DiagPlatform now has ALL mixin methods:
fn production_diagnostics(platform: &DiagPlatform) -> io::Result<()> {
    let rpm = platform.read_fan_rpm(0)?;       // from FanDiagMixin
    let tc  = platform.read_thermocouple()?;   // from TempMonitorMixin
    let ok  = platform.enable_power_rail(42)?;  // from PowerSeqMixin
    println!("Fan: {rpm} RPM, Temp: {tc}°C, Power: {ok}");
    Ok(())
}
```

### 第 5 步——使用模拟对象测试（无需硬件）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct MockSpi  { temp: Cell<f64> }
    struct MockI2c  { rpm: Cell<u32> }
    struct MockGpio { level: Cell<bool> }
    struct MockIpmi { sensor_val: Cell<f64> }

    impl SpiBus for MockSpi {
        fn spi_transfer(&self, _tx: &[u8], rx: &mut [u8]) -> io::Result<()> {
            // Encode mock temp as MAX31855 format
            let raw = ((self.temp.get() / 0.25) as i32) << 18;
            rx.copy_from_slice(&raw.to_be_bytes());
            Ok(())
        }
    }
    impl I2cBus for MockI2c {
        fn i2c_read(&self, _addr: u8, _reg: u8, buf: &mut [u8]) -> io::Result<()> {
            let tach = (self.rpm.get() / 60) as u16;
            buf.copy_from_slice(&tach.to_be_bytes());
            Ok(())
        }
        fn i2c_write(&self, _: u8, _: u8, _: &[u8]) -> io::Result<()> { Ok(()) }
    }
    impl GpioPin for MockGpio {
        fn set_high(&self)  -> io::Result<()>   { self.level.set(true);  Ok(()) }
        fn set_low(&self)   -> io::Result<()>   { self.level.set(false); Ok(()) }
        fn read_level(&self) -> io::Result<bool> { Ok(self.level.get()) }
    }
    impl IpmiBmc for MockIpmi {
        fn raw_command(&self, _: u8, _: u8, _: &[u8]) -> io::Result<Vec<u8>> { Ok(vec![]) }
        fn read_sensor(&self, _: u8) -> io::Result<f64> { Ok(self.sensor_val.get()) }
    }

    // ── Partial platform: only fan-related buses ─────────────────
    struct FanTestRig {
        i2c:  MockI2c,
        gpio: MockGpio,
    }
    impl HasI2c  for FanTestRig { type I2c  = MockI2c;  fn i2c(&self)  -> &MockI2c  { &self.i2c  } }
    impl HasGpio for FanTestRig { type Gpio = MockGpio; fn gpio(&self) -> &MockGpio { &self.gpio } }
    // FanTestRig gets FanDiagMixin but NOT TempMonitorMixin or PowerSeqMixin

    #[test]
    fn fan_health_check_passes_above_threshold() {
        let rig = FanTestRig {
            i2c:  MockI2c  { rpm: Cell::new(6000) },
            gpio: MockGpio { level: Cell::new(false) },
        };
        assert!(rig.check_fan_health(0, 4000).unwrap());
    }

    #[test]
    fn fan_health_check_fails_below_threshold() {
        let rig = FanTestRig {
            i2c:  MockI2c  { rpm: Cell::new(2000) },
            gpio: MockGpio { level: Cell::new(false) },
        };
        assert!(!rig.check_fan_health(0, 4000).unwrap());
    }
}
```

注意 `FanTestRig` 只实现了 `HasI2c + HasGpio`——它自动获得 `FanDiagMixin`，
但编译器**拒绝** `rig.read_thermocouple()`，因为 `HasSpi` 未被满足。
这就是在编译时强制执行的混入作用域控制。

### 条件方法——超越 Ruby 的能力

为单个默认方法添加 `where` 约束。该方法只在关联类型满足
额外约束时才**存在**：

```rust
/// Marker trait for DMA-capable SPI controllers
pub trait DmaCapable: SpiBus {
    fn dma_transfer(&self, tx: &[u8], rx: &mut [u8]) -> io::Result<()>;
}

/// Marker trait for interrupt-capable GPIO pins
pub trait InterruptCapable: GpioPin {
    fn wait_for_edge(&self, timeout_ms: u32) -> io::Result<bool>;
}

pub trait AdvancedDiagMixin: HasSpi + HasGpio {
    // Always available
    fn basic_probe(&self) -> io::Result<bool> {
        let mut rx = [0u8; 1];
        self.spi().spi_transfer(&[0xFF], &mut rx)?;
        Ok(rx[0] != 0x00)
    }

    // Only exists when the SPI controller supports DMA
    fn bulk_sensor_read(&self, buf: &mut [u8]) -> io::Result<()>
    where
        Self::Spi: DmaCapable,
    {
        self.spi().dma_transfer(&vec![0x00; buf.len()], buf)
    }

    // Only exists when the GPIO pin supports interrupts
    fn wait_for_fault_signal(&self, timeout_ms: u32) -> io::Result<bool>
    where
        Self::Gpio: InterruptCapable,
    {
        self.gpio().wait_for_edge(timeout_ms)
    }
}

impl<T: HasSpi + HasGpio> AdvancedDiagMixin for T {}
```

如果你的平台 SPI 不支持 DMA，调用 `bulk_sensor_read()` 是一个
**编译错误**，而不是运行时崩溃。Ruby 的 `respond_to?` 检查是最接近的
等价物——但它发生在部署时，而不是编译时。

### 可组合性：堆叠混入

多个混入可以共享同一个原料——没有菱形继承问题：

```text
┌─────────────┐    ┌───────────┐    ┌──────────────┐
│ FanDiagMixin│    │TempMonitor│    │ PowerSeqMixin│
│  (I2C+GPIO) │    │ (SPI+IPMI)│    │  (GPIO+IPMI) │
└──────┬──────┘    └─────┬─────┘    └──────┬───────┘
       │                 │                 │
       │   ┌─────────────┴─────────────┐   │
       └──►│      DiagPlatform         │◄──┘
           │ HasSpi+HasI2c+HasGpio     │
           │        +HasIpmi           │
           └───────────────────────────┘
```

`DiagPlatform` 只实现**一次** `HasGpio`，而 `FanDiagMixin` 和
`PowerSeqMixin` 都使用同一个 `self.gpio()`。在 Ruby 中，这会是两个模块
都调用 `self.gpio_pin`——但如果它们期望不同的引脚编号，你会在运行时
发现冲突。在 Rust 中，你可以在类型层面消除歧义。

### 对比：Ruby 混入 vs Rust 能力混入

| 维度 | Ruby 混入 | Rust 能力混入 |
|-----------|-------------|------------------------|
| 分派 | 运行时（方法表查找） | 编译时（单态化） |
| 安全组合 | MRO 线性化隐藏冲突 | 编译器拒绝歧义 |
| 条件方法 | 运行时 `respond_to?` | 编译时 `where` 约束 |
| 开销 | 方法分派 + GC | 零开销（内联） |
| 可测试性 | 通过元编程 stub/mock | 泛型化的模拟类型 |
| 添加新总线 | 运行时 `include` | 添加原料 trait，重新编译 |
| 运行时灵活性 | `extend`、`prepend`、开放类 | 无（完全静态） |

### 何时使用能力混入

| 场景 | 是否使用混入？ |
|----------|:-----------:|
| 多个诊断共享总线读取逻辑 | ✅ |
| 测试框架需要不同的总线子集 | ✅（部分原料结构体） |
| 方法仅对特定总线能力有效（DMA、IRQ） | ✅（条件 `where` 约束） |
| 你需要运行时模块加载（插件） | ❌（使用 `dyn Trait` 或枚举分派） |
| 单个结构体只有一条总线——无需共享 | ❌（保持简单） |
| 跨 crate 的原料有一致性问题 | ⚠️（使用 newtype 包装器） |

> **关键要点——能力混入**
>
> 1. **原料 trait（ingredient trait）** = 关联类型 + 访问器方法（如 `HasSpi`）
> 2. **混入 trait（mixin trait）** = 对原料的父 trait 约束 + 默认方法体
> 3. **泛批实现（blanket impl）** = `impl<T: HasX + HasY> Mixin for T {}`——自动注入方法
> 4. **条件方法** = 在单个默认方法上使用 `where Self::Spi: DmaCapable`
> 5. **部分平台** = 只实现所需原料的测试结构体
> 6. **无运行时开销**——编译器为每种平台类型生成专用代码

***

## 类型化命令——GADT 风格的返回类型安全

在 Haskell 中，**广义代数数据类型（GADT）**允许数据类型的每个构造器细化
类型参数——因此 `Expr Int` 和 `Expr Bool` 由类型检查器强制保证。
Rust 没有直接的 GADT 语法，但**带有关联类型的 trait**实现了相同的保证：
命令类型**决定**响应类型，混淆它们是编译错误。

这种模式对于硬件诊断特别强大，因为 IPMI 命令、寄存器读取和传感器查询
各自返回不同的物理量，绝不应该混淆。

### 问题：无类型的 `Vec<u8>` 泥潭

大多数 C/C++ IPMI 协议栈——以及简单的 Rust 移植版——到处使用原始字节：

```rust
use std::io;

struct BmcConnectionUntyped { timeout_secs: u32 }

impl BmcConnectionUntyped {
    fn raw_command(&self, net_fn: u8, cmd: u8, data: &[u8]) -> io::Result<Vec<u8>> {
        // ... shells out to ipmitool ...
        Ok(vec![0x00, 0x19, 0x00]) // stub
    }
}

fn diagnose_thermal_untyped(bmc: &BmcConnectionUntyped) -> io::Result<()> {
    // 读取 CPU 温度——传感器 ID 0x20
    let raw = bmc.raw_command(0x04, 0x2D, &[0x20])?;
    let cpu_temp = raw[0] as f64;  // 🤞 祈祷第 0 字节是读数

    // 读取风扇转速——传感器 ID 0x30
    let raw = bmc.raw_command(0x04, 0x2D, &[0x30])?;
    let fan_rpm = raw[0] as u32;  // 🐛 BUG：风扇转速是 2 字节小端

    // 读取入口电压——传感器 ID 0x40
    let raw = bmc.raw_command(0x04, 0x2D, &[0x40])?;
    let voltage = raw[0] as f64;  // 🐛 BUG：需要除以 1000

    // 🐛 将 °C 与 RPM 比较——能编译，但毫无意义
    if cpu_temp > fan_rpm as f64 {
        println!("uh oh");
    }

    // 🐛 将电压传给温度函数——编译没问题
    log_temp_untyped(voltage);
    log_volts_untyped(cpu_temp);

    Ok(())
}

fn log_temp_untyped(t: f64)  { println!("Temp: {t}°C"); }
fn log_volts_untyped(v: f64) { println!("Voltage: {v}V"); }
```

**每个读数都是 `f64`**——编译器不知道一个是温度、另一个是 RPM、又一个
是电压。四个不同的 bug 编译时没有任何警告：

| # | Bug | 后果 | 何时发现 |
|---|-----|-------------|------------|
| 1 | 风扇 RPM 解析为 1 字节而非 2 字节 | 读到 25 RPM 而非 6400 | 生产环境，凌晨 3 点风扇故障告警洪流 |
| 2 | 电压未除以 1000 | 12000V 而非 12.0V | 阈值检查标记每个电源 |
| 3 | 将 °C 与 RPM 比较 | 无意义的布尔值 | 可能永远不会 |
| 4 | 电压传给 `log_temp_untyped()` | 日志中静默数据损坏 | 6 个月后查看历史记录 |

### 解决方案：通过关联类型实现类型化命令

#### 第 1 步——领域 newtype

```rust
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Celsius(f64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Rpm(u32);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Volts(f64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Watts(f64);
```

#### 第 2 步——命令 trait（GADT 的等价物）

关联类型 `Response` 是关键——它将每个命令绑定到其返回类型：

```rust
trait IpmiCmd {
    /// The GADT "index" — determines what execute() returns.
    type Response;

    fn net_fn(&self) -> u8;
    fn cmd_byte(&self) -> u8;
    fn payload(&self) -> Vec<u8>;

    /// Parsing is encapsulated HERE — each command knows its own byte layout.
    fn parse_response(&self, raw: &[u8]) -> io::Result<Self::Response>;
}
```

#### 第 3 步——每个命令一个结构体，解析只写一次

```rust
struct ReadTemp { sensor_id: u8 }
impl IpmiCmd for ReadTemp {
    type Response = Celsius;  // ← "this command returns a temperature"
    fn net_fn(&self) -> u8 { 0x04 }
    fn cmd_byte(&self) -> u8 { 0x2D }
    fn payload(&self) -> Vec<u8> { vec![self.sensor_id] }
    fn parse_response(&self, raw: &[u8]) -> io::Result<Celsius> {
        // 按 IPMI SDR 的有符号字节——编写一次，测试一次
        Ok(Celsius(raw[0] as i8 as f64))
    }
}

struct ReadFanSpeed { fan_id: u8 }
impl IpmiCmd for ReadFanSpeed {
    type Response = Rpm;     // ← "this command returns RPM"
    fn net_fn(&self) -> u8 { 0x04 }
    fn cmd_byte(&self) -> u8 { 0x2D }
    fn payload(&self) -> Vec<u8> { vec![self.fan_id] }
    fn parse_response(&self, raw: &[u8]) -> io::Result<Rpm> {
        // 2 字节小端——正确的布局，编码一次
        Ok(Rpm(u16::from_le_bytes([raw[0], raw[1]]) as u32))
    }
}

struct ReadVoltage { rail: u8 }
impl IpmiCmd for ReadVoltage {
    type Response = Volts;   // ← "this command returns voltage"
    fn net_fn(&self) -> u8 { 0x04 }
    fn cmd_byte(&self) -> u8 { 0x2D }
    fn payload(&self) -> Vec<u8> { vec![self.rail] }
    fn parse_response(&self, raw: &[u8]) -> io::Result<Volts> {
        // 毫伏 → 伏特，始终正确
        Ok(Volts(u16::from_le_bytes([raw[0], raw[1]]) as f64 / 1000.0))
    }
}

struct ReadFru { fru_id: u8 }
impl IpmiCmd for ReadFru {
    type Response = String;
    fn net_fn(&self) -> u8 { 0x0A }
    fn cmd_byte(&self) -> u8 { 0x11 }
    fn payload(&self) -> Vec<u8> { vec![self.fru_id, 0x00, 0x00, 0xFF] }
    fn parse_response(&self, raw: &[u8]) -> io::Result<String> {
        Ok(String::from_utf8_lossy(raw).to_string())
    }
}
```

#### 第 4 步——执行器（零 `dyn`，单态化）

```rust
struct BmcConnection { timeout_secs: u32 }

impl BmcConnection {
    /// Generic over any command — compiler generates one version per command type.
    fn execute<C: IpmiCmd>(&self, cmd: &C) -> io::Result<C::Response> {
        let raw = self.raw_send(cmd.net_fn(), cmd.cmd_byte(), &cmd.payload())?;
        cmd.parse_response(&raw)
    }

    fn raw_send(&self, _nf: u8, _cmd: u8, _data: &[u8]) -> io::Result<Vec<u8>> {
        Ok(vec![0x19, 0x00]) // stub — real impl calls ipmitool
    }
}
```

#### 第 5 步——调用方代码：四个 bug 全部变成编译错误

```rust
fn diagnose_thermal(bmc: &BmcConnection) -> io::Result<()> {
    let cpu_temp: Celsius = bmc.execute(&ReadTemp { sensor_id: 0x20 })?;
    let fan_rpm:  Rpm     = bmc.execute(&ReadFanSpeed { fan_id: 0x30 })?;
    let voltage:  Volts   = bmc.execute(&ReadVoltage { rail: 0x40 })?;

    // Bug #1 — 不可能：解析逻辑在 ReadFanSpeed::parse_response 中
    // Bug #2 — 不可能：缩放逻辑在 ReadVoltage::parse_response 中

    // Bug #3 — 编译错误：
    // if cpu_temp > fan_rpm { }
    //    ^^^^^^^^   ^^^^^^^
    //    Celsius    Rpm      → "mismatched types"（类型不匹配）❌

    // Bug #4 — 编译错误：
    // log_temperature(voltage);
    //                 ^^^^^^^  Volts，期望 Celsius ❌

    // 只有正确的比较才能编译：
    if cpu_temp > Celsius(85.0) {
        println!("CPU overheating: {:?}", cpu_temp);
    }
    if fan_rpm < Rpm(4000) {
        println!("Fan too slow: {:?}", fan_rpm);
    }

    Ok(())
}

fn log_temperature(t: Celsius) { println!("Temp: {:?}", t); }
fn log_voltage(v: Volts)       { println!("Voltage: {:?}", v); }
```

### 用于诊断脚本的宏 DSL

对于运行大量命令的大型诊断例程，宏能提供简洁的声明式语法，
同时保持完全的类型安全：

```rust
/// Execute a series of typed IPMI commands, returning a tuple of results.
/// Each element of the tuple has the command's own Response type.
macro_rules! diag_script {
    ($bmc:expr; $($cmd:expr),+ $(,)?) => {{
        ( $( $bmc.execute(&$cmd)?, )+ )
    }};
}

fn full_pre_flight(bmc: &BmcConnection) -> io::Result<()> {
    // Expands to: (Celsius, Rpm, Volts, String) — every type tracked
    let (temp, rpm, volts, board_pn) = diag_script!(bmc;
        ReadTemp     { sensor_id: 0x20 },
        ReadFanSpeed { fan_id:    0x30 },
        ReadVoltage  { rail:      0x40 },
        ReadFru      { fru_id:    0x00 },
    );

    println!("Board: {:?}", board_pn);
    println!("CPU: {:?}, Fan: {:?}, 12V: {:?}", temp, rpm, volts);

    // Type-safe threshold checks:
    assert!(temp  < Celsius(95.0), "CPU too hot");
    assert!(rpm   > Rpm(3000),     "Fan too slow");
    assert!(volts > Volts(11.4),   "12V rail sagging");

    Ok(())
}
```

这个宏只是语法糖——元组类型 `(Celsius, Rpm, Volts, String)` 完全由
编译器推断。交换两个命令，解构会在编译时失败，而不是在运行时。

### 异构命令列表的枚举分派

当你需要一个混合命令的 `Vec`（例如从 JSON 加载的可配置脚本）时，
使用枚举分派来保持无 `dyn`：

```rust
enum AnyReading {
    Temp(Celsius),
    Rpm(Rpm),
    Volt(Volts),
    Text(String),
}

enum AnyCmd {
    Temp(ReadTemp),
    Fan(ReadFanSpeed),
    Voltage(ReadVoltage),
    Fru(ReadFru),
}

impl AnyCmd {
    fn execute(&self, bmc: &BmcConnection) -> io::Result<AnyReading> {
        match self {
            AnyCmd::Temp(c)    => Ok(AnyReading::Temp(bmc.execute(c)?)),
            AnyCmd::Fan(c)     => Ok(AnyReading::Rpm(bmc.execute(c)?)),
            AnyCmd::Voltage(c) => Ok(AnyReading::Volt(bmc.execute(c)?)),
            AnyCmd::Fru(c)     => Ok(AnyReading::Text(bmc.execute(c)?)),
        }
    }
}

/// Dynamic diagnostic script — commands loaded at runtime
fn run_script(bmc: &BmcConnection, script: &[AnyCmd]) -> io::Result<Vec<AnyReading>> {
    script.iter().map(|cmd| cmd.execute(bmc)).collect()
}
```

你失去了逐元素的类型跟踪（一切都变成 `AnyReading`），但获得了
运行时灵活性——而且解析仍然封装在每个 `IpmiCmd` 实现中。

### 测试类型化命令

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct StubBmc {
        responses: std::collections::HashMap<u8, Vec<u8>>,
    }

    impl StubBmc {
        fn execute<C: IpmiCmd>(&self, cmd: &C) -> io::Result<C::Response> {
            let key = cmd.payload()[0]; // sensor ID as key
            let raw = self.responses.get(&key)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no stub"))?;
            cmd.parse_response(raw)
        }
    }

    #[test]
    fn read_temp_parses_signed_byte() {
        let bmc = StubBmc {
            responses: [( 0x20, vec![0xE7] )].into() // -25 as i8 = 0xE7
        };
        let temp = bmc.execute(&ReadTemp { sensor_id: 0x20 }).unwrap();
        assert_eq!(temp, Celsius(-25.0));
    }

    #[test]
    fn read_fan_parses_two_byte_le() {
        let bmc = StubBmc {
            responses: [( 0x30, vec![0x00, 0x19] )].into() // 0x1900 = 6400
        };
        let rpm = bmc.execute(&ReadFanSpeed { fan_id: 0x30 }).unwrap();
        assert_eq!(rpm, Rpm(6400));
    }

    #[test]
    fn read_voltage_scales_millivolts() {
        let bmc = StubBmc {
            responses: [( 0x40, vec![0xE8, 0x2E] )].into() // 0x2EE8 = 12008 mV
        };
        let v = bmc.execute(&ReadVoltage { rail: 0x40 }).unwrap();
        assert!((v.0 - 12.008).abs() < 0.001);
    }
}
```

每个命令的解析都独立测试。如果 `ReadFanSpeed` 在新的 IPMI 规范版本中
从 2 字节 LE 变为 4 字节 BE，你只需更新**一个** `parse_response`，
测试就能捕获回归。

### 这如何映射到 Haskell GADT

```text
Haskell GADT                         Rust Equivalent
────────────────                     ───────────────────────
data Cmd a where                     trait IpmiCmd {
  ReadTemp :: SensorId -> Cmd Temp       type Response;
  ReadFan  :: FanId    -> Cmd Rpm        ...
                                     }

eval :: Cmd a -> IO a                fn execute<C: IpmiCmd>(&self, cmd: &C)
                                         -> io::Result<C::Response>

Type refinement in case branches     Monomorphisation: compiler generates
                                     execute::<ReadTemp>() → returns Celsius
                                     execute::<ReadFanSpeed>() → returns Rpm
```

两者都保证：**命令决定返回类型**。Rust 通过泛型单态化而非类型层面的
case 分析来实现这一点——同样的安全性，零运行时开销。

### 改造前后对比

| 维度 | 无类型（`Vec<u8>`） | 类型化命令 |
|-----------|:---:|:---:|
| 每个传感器代码行数 | 约 3 行（在每个调用点重复） | 约 15 行（编写并测试一次） |
| 解析错误可能性 | 每个调用点 | 在一个 `parse_response` 实现中 |
| 单位混淆 bug | 无限 | 零（编译错误） |
| 添加新传感器 | 修改 N 个文件，复制粘贴解析 | 添加 1 个结构体 + 1 个实现 |
| 运行时开销 | — | 相同（单态化） |
| IDE 自动补全 | 到处都是 `f64` | `Celsius`、`Rpm`、`Volts`——自文档化 |
| 代码审查负担 | 必须验证每个原始字节解析 | 每个传感器验证一个 `parse_response` |
| 宏 DSL | 不适用 | `diag_script!(bmc; ReadTemp{..}, ReadFan{..})` → `(Celsius, Rpm)` |
| 动态脚本 | 手动分派 | `AnyCmd` 枚举——仍然无 `dyn` |

### 何时使用类型化命令

| 场景 | 推荐 |
|----------|:--------------:|
| 具有不同物理单位的 IPMI 传感器读取 | ✅ 类型化命令 |
| 具有不同宽度字段的寄存器映射 | ✅ 类型化命令 |
| 网络协议消息（请求 → 响应） | ✅ 类型化命令 |
| 单一命令类型且只有一种返回格式 | ❌ 杀鸡用牛刀——直接返回类型即可 |
| 原型设计/探索未知设备 | ❌ 先用原始字节，之后再类型化 |
| 命令在编译时未知的插件系统 | ⚠️ 使用 `AnyCmd` 枚举分派 |

> **关键要点——Trait**
> - 关联类型 = 每种类型一个实现；泛型参数 = 每种类型多个实现
> - GAT 解锁了借贷迭代器和 trait 中的 async 模式
> - 封闭集合使用枚举分派（快速）；开放集合使用 `dyn Trait`（灵活）
> - `Any` + `TypeId` 是编译时类型未知时的逃生舱

> **另请参阅：**[第 1 章——泛型](ch01-generics-the-full-picture.md)了解单态化以及泛型何时导致代码膨胀。[第 3 章——newtype 与类型状态](ch03-the-newtype-and-type-state-patterns.md)了解如何将 trait 与 config trait 模式配合使用。

---

### 练习：带关联类型的 Repository ★★★（约 40 分钟）

设计一个 `Repository` trait，带有关联的 `Error`、`Id` 和 `Item` 类型。为内存存储实现它，并演示编译时类型安全。

<details>
<summary>🔑 答案</summary>

```rust
use std::collections::HashMap;

trait Repository {
    type Item;
    type Id;
    type Error;

    fn get(&self, id: &Self::Id) -> Result<Option<&Self::Item>, Self::Error>;
    fn insert(&mut self, item: Self::Item) -> Result<Self::Id, Self::Error>;
    fn delete(&mut self, id: &Self::Id) -> Result<bool, Self::Error>;
}

#[derive(Debug, Clone)]
struct User {
    name: String,
    email: String,
}

struct InMemoryUserRepo {
    data: HashMap<u64, User>,
    next_id: u64,
}

impl InMemoryUserRepo {
    fn new() -> Self {
        InMemoryUserRepo { data: HashMap::new(), next_id: 1 }
    }
}

impl Repository for InMemoryUserRepo {
    type Item = User;
    type Id = u64;
    type Error = std::convert::Infallible;

    fn get(&self, id: &u64) -> Result<Option<&User>, Self::Error> {
        Ok(self.data.get(id))
    }

    fn insert(&mut self, item: User) -> Result<u64, Self::Error> {
        let id = self.next_id;
        self.next_id += 1;
        self.data.insert(id, item);
        Ok(id)
    }

    fn delete(&mut self, id: &u64) -> Result<bool, Self::Error> {
        Ok(self.data.remove(id).is_some())
    }
}

fn create_and_fetch<R: Repository>(repo: &mut R, item: R::Item) -> Result<(), R::Error>
where
    R::Item: std::fmt::Debug,
    R::Id: std::fmt::Debug,
{
    let id = repo.insert(item)?;
    println!("Inserted with id: {id:?}");
    let retrieved = repo.get(&id)?;
    println!("Retrieved: {retrieved:?}");
    Ok(())
}

fn main() {
    let mut repo = InMemoryUserRepo::new();
    create_and_fetch(&mut repo, User {
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).unwrap();
}
```

</details>

***
