# 13. 宏——编写代码的代码 🟡

> **你将学到：**
> - 声明式宏（`macro_rules!`），包括模式匹配与重复
> - 宏与泛型/trait 相比，何时是正确的工具
> - 过程宏（procedural macros）：派生宏、属性宏和函数式宏
> - 使用 `syn` 和 `quote` 编写自定义派生宏

## 声明式宏（macro_rules!）

宏在语法上进行模式匹配，并在编译期展开为代码：

```rust
// 一个简单的创建 HashMap 的宏
macro_rules! hashmap {
    // 匹配：以逗号分隔的 key => value 键值对
    ( $( $key:expr => $value:expr ),* $(,)? ) => {
        {
            let mut map = std::collections::HashMap::new();
            $( map.insert($key, $value); )*
            map
        }
    };
}

let scores = hashmap! {
    "Alice" => 95,
    "Bob" => 87,
    "Carol" => 92,
};
// 展开为：
// let mut map = HashMap::new();
// map.insert("Alice", 95);
// map.insert("Bob", 87);
// map.insert("Carol", 92);
// map
```

**宏片段类型**：

| 片段 | 匹配 | 示例 |
|----------|---------|---------|
| `$x:expr` | 任意表达式 | `42`, `a + b`, `foo()` |
| `$x:ty` | 一个类型 | `i32`, `Vec<String>` |
| `$x:ident` | 一个标识符 | `my_var`, `Config` |
| `$x:pat` | 一个模式 | `Some(x)`, `_` |
| `$x:stmt` | 一条语句 | `let x = 5;` |
| `$x:tt` | 单个 token 树 | 任何内容（最灵活） |
| `$x:literal` | 一个字面量 | `42`, `"hello"`, `true` |

**重复**：`$( ... ),*` 表示"零个或多个，以逗号分隔"

```rust
// 自动生成测试函数
macro_rules! test_cases {
    ( $( $name:ident: $input:expr => $expected:expr ),* $(,)? ) => {
        $(
            #[test]
            fn $name() {
                assert_eq!(process($input), $expected);
            }
        )*
    };
}

test_cases! {
    test_empty: "" => "",
    test_hello: "hello" => "HELLO",
    test_trim: "  spaces  " => "SPACES",
}
// 生成三个独立的 #[test] 函数
```

### 何时（不）使用宏

**在以下情况使用宏**：
- 减少 trait/泛型无法处理的样板代码（可变参数、DRY 测试生成）
- 创建 DSL（`html!`、`sql!`、`vec!`）
- 条件代码生成（`cfg!`、`compile_error!`）

**在以下情况不要使用宏**：
- 函数或泛型就能工作时（宏更难调试，自动补全也不起作用）
- 你需要在宏内部进行类型检查（宏操作的是 token，而非类型）
- 模式只用一两次（不值得付出抽象成本）

```rust
// ❌ 不必要的宏——函数就够了：
macro_rules! double {
    ($x:expr) => { $x * 2 };
}

// ✅ 直接用函数即可：
fn double(x: i32) -> i32 { x * 2 }

// ✅ 好的宏用法——可变参数，无法用函数实现：
macro_rules! println {
    ($($arg:tt)*) => { /* format string + args */ };
}
```

### 过程宏概述

过程宏是转换 token 流的 Rust 函数。它们需要一个单独的 crate，并设置 `proc-macro = true`：

```rust
// 三种过程宏：

// 1. 派生宏 — #[derive(MyTrait)]
// 从结构体定义生成 trait 实现
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    name: String,
    port: u16,
}

// 2. 属性宏 — #[my_attribute]
// 转换被标注的条目
#[route(GET, "/api/users")]
async fn list_users() -> Json<Vec<User>> { /* ... */ }

// 3. 函数式宏 — my_macro!(...)
// 自定义语法
let query = sql!(SELECT * FROM users WHERE id = ?);
```

### 派生宏实践

最常见的 proc 宏类型。以下是 `#[derive(Debug)]` 在概念上是如何工作的：

```rust
// 输入（你的结构体）：
#[derive(Debug)]
struct Point {
    x: f64,
    y: f64,
}

// 派生宏生成：
impl std::fmt::Debug for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}
```

**常用的派生宏**：

| 派生 | Crate | 生成内容 |
|--------|-------|-------------------|
| `Debug` | std | `fmt::Debug` 实现（调试打印） |
| `Clone`、`Copy` | std | 值复制 |
| `PartialEq`、`Eq` | std | 相等性比较 |
| `Hash` | std | 用于 HashMap 键的哈希 |
| `Serialize`、`Deserialize` | serde | JSON/YAML 等编码 |
| `Error` | thiserror | `std::error::Error` + `Display` |
| `Parser` | `clap` | CLI 参数解析 |
| `Builder` | derive_builder | 构建器模式 |

> **实用建议**：大胆使用派生宏——它们消除了容易出错的样板代码。编写自己的
> proc 宏是一个高级主题；在构建自定义宏之前，先使用现有的（`serde`、
> `thiserror`、`clap`）。

### 宏的卫生性与 `$crate`

**卫生性（Hygiene）**意味着宏内部创建的标识符不会与调用者作用域中的标识符冲突。Rust 的 `macro_rules!` 是*部分*卫生的：

```rust
macro_rules! make_var {
    () => {
        let x = 42; // 这个 'x' 在宏的作用域中
    };
}

fn main() {
    let x = 10;
    make_var!();   // 创建了一个不同的 'x'（卫生的）
    println!("{x}"); // 打印 10，而非 42——宏的 x 不会泄漏
}
```

**`$crate`**：在库中编写宏时，使用 `$crate` 引用你自己的 crate——无论用户如何导入你的 crate，它都能正确解析：

```rust
// 在 my_diagnostics crate 中：

pub fn log_result(msg: &str) {
    println!("[diag] {msg}");
}

#[macro_export]
macro_rules! diag_log {
    ($($arg:tt)*) => {
        // ✅ $crate 总是解析为 my_diagnostics，即使用户
        // 在 Cargo.toml 中重命名了该 crate
        $crate::log_result(&format!($($arg)*))
    };
}

// ❌ 不使用 $crate：
// my_diagnostics::log_result(...)  ← 在用户如下配置时会失效：
//   [dependencies]
//   diag = { package = "my_diagnostics", version = "1" }
```

> **规则**：在 `#[macro_export]` 宏中始终使用 `$crate::`。永远不要直接使用
> 你的 crate 名称。

### 递归宏与 `tt` 消化

递归宏一次处理一个 token——这种技术称为 **`tt` 消化**（token-tree munching）：

```rust
// 统计传递给宏的表达式数量
macro_rules! count {
    // 基本情况：没有剩余 token
    () => { 0usize };
    // 递归情况：消费一个表达式，统计剩余的
    ($head:expr $(, $tail:expr)* $(,)?) => {
        1usize + count!($($tail),*)
    };
}

fn main() {
    let n = count!("a", "b", "c", "d");
    assert_eq!(n, 4);

    // 也可以在编译期使用：
    const N: usize = count!(1, 2, 3);
    assert_eq!(N, 3);
}
```

```rust
// 从表达式列表构建异构元组：
macro_rules! tuple_from {
    // 基本情况：单个元素
    ($single:expr $(,)?) => { ($single,) };
    // 递归情况：第一个元素 + 剩余部分
    ($head:expr, $($tail:expr),+ $(,)?) => {
        ($head, tuple_from!($($tail),+))
    };
}

let t = tuple_from!(1, "hello", 3.14, true);
// 展开为：(1, ("hello", (3.14, (true,))))
```

**片段说明符的细微之处**：

| 片段 | 注意事项 |
|----------|--------|
| `$x:expr` | 贪婪解析——`1 + 2` 是一个表达式，不是三个 token |
| `$x:ty` | 贪婪解析——`Vec<String>` 是一个类型；后面不能跟 `+` 或 `<` |
| `$x:tt` | 精确匹配一个 token 树——最灵活，检查最少 |
| `$x:ident` | 仅匹配普通标识符——不能是 `std::io` 这样的路径 |
| `$x:pat` | 在 Rust 2021 中，匹配 `A \| B` 模式；对单一模式使用 `$x:pat_param` |

> **何时使用 `tt`**：当你需要将 token 转发给另一个宏而不受解析器约束时。
> `$($args:tt)*` 是"接受一切"的模式（被 `println!`、`format!`、`vec!` 使用）。

### 使用 `syn` 和 `quote` 编写派生宏

派生宏位于单独的 crate（`proc-macro = true`）中，使用 `syn`（解析 Rust）和 `quote`（生成 Rust）来转换 token 流：

```toml
# my_derive/Cargo.toml
[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"
```

```rust
// my_derive/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// 派生宏，生成一个返回结构体名称和字段名的 `describe()` 方法
#[proc_macro_derive(Describe)]
pub fn derive_describe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    // 提取字段名（仅适用于具名字段的结构体）
    let fields = match &input.data {
        syn::Data::Struct(data) => {
            data.fields.iter()
                .filter_map(|f| f.ident.as_ref())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
        }
        _ => vec![],
    };

    let field_list = fields.join(", ");

    let expanded = quote! {
        impl #name {
            pub fn describe() -> String {
                format!("{} {{ {} }}", #name_str, #field_list)
            }
        }
    };

    TokenStream::from(expanded)
}
```

```rust
// 在应用 crate 中：
use my_derive::Describe;

#[derive(Describe)]
struct SensorReading {
    sensor_id: u16,
    value: f64,
    timestamp: u64,
}

fn main() {
    println!("{}", SensorReading::describe());
    // "SensorReading { sensor_id, value, timestamp }"
}
```

**工作流程**：`TokenStream`（原始 token）→ `syn::parse`（AST）→
检查/转换 → `quote!`（生成 token）→ `TokenStream`（返回编译器）。

| Crate | 角色 | 关键类型 |
|-------|------|-----------|
| `proc-macro` | 编译器接口 | `TokenStream` |
| `syn` | 将 Rust 源码解析为 AST | `DeriveInput`、`ItemFn`、`Type` |
| `quote` | 从模板生成 Rust token | `quote!{}`、`#variable` 插值 |
| `proc-macro2` | syn/quote 与 proc-macro 之间的桥梁 | `TokenStream`、`Span` |

> **实用技巧**：在编写自己的宏之前，先研究像 `thiserror` 或 `derive_more` 这样
> 简单的派生宏源码。`cargo expand` 命令（通过 `cargo-expand`）能展示任何宏展开后的
> 结果——对调试极有帮助。

> **关键要点——宏**
> - 简单代码生成用 `macro_rules!`；复杂的派生用 proc 宏（`syn` + `quote`）
> - 尽可能优先使用泛型/trait 而非宏——宏更难调试和维护
> - `$crate` 确保卫生性；`tt` 消化实现递归模式匹配

> **另请参阅：**[第 2 章——Trait](ch02-traits-in-depth.md)了解 trait/泛型何时优于宏。[第 13 章——测试](ch14-testing-and-benchmarking-patterns.md)了解如何测试宏生成的代码。

```mermaid
flowchart LR
    A["源代码"] --> B["macro_rules!<br>模式匹配"]
    A --> C["#[derive(MyMacro)]<br>过程宏"]

    B --> D["Token 展开"]
    C --> E["syn：解析 AST"]
    E --> F["转换"]
    F --> G["quote!：生成 token"]
    G --> D

    D --> H["编译后代码"]

    style A fill:#e8f4f8,stroke:#2980b9,color:#000
    style B fill:#d4efdf,stroke:#27ae60,color:#000
    style C fill:#fdebd0,stroke:#e67e22,color:#000
    style D fill:#fef9e7,stroke:#f1c40f,color:#000
    style E fill:#fdebd0,stroke:#e67e22,color:#000
    style F fill:#fdebd0,stroke:#e67e22,color:#000
    style G fill:#fdebd0,stroke:#e67e22,color:#000
    style H fill:#d4efdf,stroke:#27ae60,color:#000
```

---

### 练习：声明式宏——`map!` ★（约 15 分钟）

编写一个 `map!` 宏，从键值对创建 `HashMap`：

```rust,ignore
let m = map! {
    "host" => "localhost",
    "port" => "8080",
};
assert_eq!(m.get("host"), Some(&"localhost"));
```

要求：支持尾随逗号和空调用 `map!{}`。

<details>
<summary>🔑 解答</summary>

```rust
macro_rules! map {
    () => { std::collections::HashMap::new() };
    ( $( $key:expr => $val:expr ),+ $(,)? ) => {{
        let mut m = std::collections::HashMap::new();
        $( m.insert($key, $val); )+
        m
    }};
}

fn main() {
    let config = map! {
        "host" => "localhost",
        "port" => "8080",
        "timeout" => "30",
    };
    assert_eq!(config.len(), 3);
    assert_eq!(config["host"], "localhost");

    let empty: std::collections::HashMap<String, String> = map!();
    assert!(empty.is_empty());

    let scores = map! { 1 => 100, 2 => 200 };
    assert_eq!(scores[&1], 100);
}
```

</details>

***
