# 15. Crate 架构与 API 设计 🟡

> **你将学到：**
> - 模块布局约定与再导出策略
> - 精打磨 crate 的公共 API 设计清单
> - 符合人体工程学的参数模式：`impl Into`、`AsRef`、`Cow`
> - 使用 `TryFrom` 与已验证类型实践"解析，而非校验"
> - 特性开关（feature flags）、条件编译与工作区组织

## 模块布局约定

```text
my_crate/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Crate 根 — 再导出与公共 API
│   ├── config.rs       # 功能模块
│   ├── parser/         # 包含子模块的复杂模块
│   │   ├── mod.rs      # 或在父级使用 parser.rs（Rust 2018+）
│   │   ├── lexer.rs
│   │   └── ast.rs
│   ├── error.rs        # 错误类型
│   └── utils.rs        # 内部辅助函数（pub(crate)）
├── tests/
│   └── integration.rs  # 集成测试
├── benches/
│   └── perf.rs         # 基准测试
└── examples/
    └── basic.rs        # cargo run --example basic
```

```rust
// lib.rs — 通过再导出来精心组织你的公共 API：
mod config;
mod error;
mod parser;
mod utils;

// 再导出用户所需的内容：
pub use config::Config;
pub use error::Error;
pub use parser::Parser;

// 公共类型位于 crate 根 — 用户这样写：
// use my_crate::Config;
// 而不是：use my_crate::config::Config;
```

**可见性修饰符**：

| 修饰符 | 对谁可见 |
|----------|-----------|
| `pub` | 所有人 |
| `pub(crate)` | 仅本 crate |
| `pub(super)` | 父模块 |
| `pub(in path)` | 特定的祖先模块 |
| （无） | 当前模块及其子模块 |

### 公共 API 设计清单

1. **接受引用，返回拥有类型** — `fn process(input: &str) -> String`
2. **参数使用 `impl Trait`** — `fn read(r: impl Read)` 比 `fn read<R: Read>(r: R)` 签名更简洁
3. **返回 `Result`，不要 `panic!`** — 让调用者决定如何处理错误
4. **实现标准 trait** — `Debug`、`Display`、`Clone`、`Default`、`From`/`Into`
5. **让非法状态不可表示** — 使用类型状态（type state）和新类型模式（newtype）
6. **复杂配置遵循建造者模式（builder pattern）** — 若有必填字段则配合类型状态
7. **密封（seal）你不希望用户实现的 trait** — `pub trait Sealed: private::Sealed {}`
8. **为类型和函数标注 `#[must_use]`** — 防止静默丢弃重要的 `Result`、守卫（guard）或值。对任何"忽略返回值几乎肯定是个 bug"的类型都应加上：
   ```rust
   #[must_use = "dropping the guard immediately releases the lock"]
   pub struct LockGuard<'a, T> { /* ... */ }

   #[must_use]
   pub fn validate(input: &str) -> Result<ValidInput, ValidationError> { /* ... */ }
   ```

```rust
// 密封 trait 模式 — 用户可以使用但不能实现：
mod private {
    pub trait Sealed {}
}

pub trait DatabaseDriver: private::Sealed {
    fn connect(&self, url: &str) -> Connection;
}

// 只有本 crate 中的类型才能实现 Sealed → 只有我们能实现 DatabaseDriver
pub struct PostgresDriver;
impl private::Sealed for PostgresDriver {}
impl DatabaseDriver for PostgresDriver {
    fn connect(&self, url: &str) -> Connection { /* ... */ }
}
```

> **`#[non_exhaustive]`** — 标记公共枚举和结构体，这样添加变体或字段就不是破坏性变更。下游 crate 必须在 match 语句中使用通配分支（`_ =>`），且不能用结构体字面量语法构造该类型：
> ```rust
> #[non_exhaustive]
> pub enum DiagError {
>     Timeout,
>     HardwareFault,
>     // 在未来版本中添加新变体不是语义版本破坏。
> }
> ```

### 符合人体工程学的参数模式 — `impl Into`、`AsRef`、`Cow`

Rust 最具影响力的 API 模式之一是在函数参数中接受**最通用的类型**，这样调用者就不需要在每个调用点反复写 `.to_string()`、`&*s` 或 `.as_ref()`。这是 Rust 版的"对你接受的东西保持宽容"。

#### `impl Into<T>` — 接受任何可转换的类型

```rust
// ❌ 摩擦：调用者必须手动转换
fn connect(host: String, port: u16) -> Connection {
    // ...
}
connect("localhost".to_string(), 5432);  // 烦人的 .to_string()
connect(hostname.clone(), 5432);          // 如果已有 String，这是不必要的 clone

// ✅ 符合人体工程学：接受任何可转换为 String 的类型
fn connect(host: impl Into<String>, port: u16) -> Connection {
    let host = host.into();  // 在函数内部转换一次
    // ...
}
connect("localhost", 5432);     // &str — 零摩擦
connect(hostname, 5432);        // String — 移动，无 clone
```

之所以可行，是因为 Rust 的 `From`/`Into` trait 对提供了通用转换。当你接受 `impl Into<T>` 时，你的意思是："给我任何知道如何变成 `T` 的东西。"

#### `AsRef<T>` — 以引用借用

`AsRef<T>` 是 `Into<T>` 的借用对应物。当你只需要*读取*数据，而不是获取所有权时使用它：

```rust
use std::path::Path;

// ❌ 强制调用者转换为 &Path
fn file_exists(path: &Path) -> bool {
    path.exists()
}
file_exists(Path::new("/tmp/test.txt"));  // 尴尬

// ✅ 接受任何可以表现为 &Path 的类型
fn file_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}
file_exists("/tmp/test.txt");                    // &str ✅
file_exists(String::from("/tmp/test.txt"));      // String ✅
file_exists(Path::new("/tmp/test.txt"));         // &Path ✅
file_exists(PathBuf::from("/tmp/test.txt"));     // PathBuf ✅

// 同样的模式适用于类字符串参数：
fn log_message(msg: impl AsRef<str>) {
    println!("[LOG] {}", msg.as_ref());
}
log_message("hello");                    // &str ✅
log_message(String::from("hello"));      // String ✅
```

#### `Cow<T>` — 写入时克隆

`Cow<'a, T>`（Clone on Write，写入时克隆）将分配延迟到需要修改时。它要么持有借用的 `&T`，要么持有拥有的 `T::Owned`。当大多数调用不需要修改数据时非常完美：

```rust
use std::borrow::Cow;

/// 归一化诊断消息 — 仅在需要修改时才分配。
fn normalize_message(msg: &str) -> Cow<'_, str> {
    if msg.contains('\t') || msg.contains('\r') {
        // 必须分配 — 我们需要修改内容
        Cow::Owned(msg.replace('\t', "    ").replace('\r', ""))
    } else {
        // 无分配 — 仅借用原始数据
        Cow::Borrowed(msg)
    }
}

// 大多数消息无需分配即可通过：
let clean = normalize_message("All tests passed");          // Borrowed — 免费
let fixed = normalize_message("Error:\tfailed\r\n");        // Owned — 已分配

// Cow<str> 实现了 Deref<Target=str>，所以它表现得像 &str：
println!("{}", clean);
println!("{}", fixed.to_uppercase());
```

#### 快速参考：该用哪个

```text
你需要函数内部数据的所有权吗？
├── 是 → impl Into<T>
│        "给我任何能变成 T 的东西"
└── 否  → 你只需要读取它吗？
     ├── 是 → impl AsRef<T> 或 &T
     │        "给我任何能借用为 &T 的东西"
     └── 也许（有时可能需要修改？）
          └── Cow<'_, T>
              "尽可能借用，只在必须时才克隆"
```

| 模式 | 所有权 | 分配 | 何时使用 |
|---------|-----------|------------|-------------|
| `&str` | 借用 | 从不 | 简单字符串参数 |
| `impl AsRef<str>` | 借用 | 从不 | 接受 String、&str 等 — 只读 |
| `impl Into<String>` | 拥有 | 转换时 | 接受 &str、String — 将存储/拥有 |
| `Cow<'_, str>` | 二者皆可 | 仅修改时 | 通常不修改的处理 |
| `&[u8]` / `impl AsRef<[u8]>` | 借用 | 从不 | 面向字节的 API |

> **`Borrow<T>` vs `AsRef<T>`**：二者都提供 `&T`，但 `Borrow<T>` 额外保证 `Eq`、`Ord` 和 `Hash` 在原始形式与借用形式之间**一致**。这就是为什么 `HashMap<String, V>::get()` 接受 `&Q where String: Borrow<Q>` — 而不是 `AsRef`。当借用形式用作查找键时使用 `Borrow`；对于通用的"给我一个引用"参数使用 `AsRef`。

#### 在 API 中组合转换

```rust
/// 一个设计良好的诊断 API，使用符合人体工程学的参数：
pub struct DiagRunner {
    name: String,
    config_path: PathBuf,
    results: HashMap<String, TestResult>,
}

impl DiagRunner {
    /// name 接受任何类字符串类型，config 接受任何类路径类型。
    pub fn new(
        name: impl Into<String>,
        config_path: impl Into<PathBuf>,
    ) -> Self {
        DiagRunner {
            name: name.into(),
            config_path: config_path.into(),
        }
    }

    /// 只读查找时接受任何 AsRef<str>。
    pub fn get_result(&self, test_name: impl AsRef<str>) -> Option<&TestResult> {
        self.results.get(test_name.as_ref())
    }
}

// 所有这些都能零调用摩擦地工作：
let runner = DiagRunner::new("GPU Diag", "/etc/diag_tool/config.json");
let runner = DiagRunner::new(format!("Diag-{}", node_id), config_path);
let runner = DiagRunner::new(name_string, path_buf);
```

***

## 案例研究：设计公共 Crate API — 改造前后

这是一个真实的例子，展示如何将一个"字符串类型"的内部 API 演进为符合人体工程学、类型安全的公共 API。考虑一个配置解析器 crate：

**改造前**（字符串类型，容易误用）：

```rust
// ❌ 所有参数都是字符串 — 没有编译期校验
pub fn parse_config(path: &str, format: &str, strict: bool) -> Result<Config, String> {
    // 哪些格式是有效的？"json"？"JSON"？"Json"？
    // path 是文件路径还是 URL？
    // "strict" 到底是什么意思？
    todo!()
}
```

**改造后**（类型安全，自文档化）：

```rust
use std::path::Path;

/// 支持的配置格式。
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]  // 添加格式不会破坏下游
pub enum Format {
    Json,
    Toml,
    Yaml,
}

/// 控制解析严格程度。
#[derive(Debug, Clone, Copy, Default)]
pub enum Strictness {
    /// 拒绝未知字段（库的默认行为）
    #[default]
    Strict,
    /// 忽略未知字段（对前向兼容的配置有用）
    Lenient,
}

pub fn parse_config(
    path: &Path,          // 类型强制：必须是文件系统路径
    format: Format,       // 枚举：不可能传入无效格式
    strictness: Strictness,  // 命名的替代项，而非裸 bool
) -> Result<Config, ConfigError> {
    todo!()
}
```

**改进了什么**：

| 方面 | 改造前 | 改造后 |
|--------|--------|-------|
| 格式校验 | 运行时字符串比较 | 编译期枚举 |
| 路径类型 | 裸 `&str`（可以是任何东西） | `&Path`（文件系统专用） |
| 严格度 | 神秘的 `bool` | 自文档化的枚举 |
| 错误类型 | `String`（不透明） | `ConfigError`（结构化） |
| 可扩展性 | 破坏性变更 | `#[non_exhaustive]` |

> **经验法则**：如果你发现自己对字符串值写 `match`，考虑用枚举替换该参数。如果某个参数是一个在上下文中含义不明的布尔值，使用一个双变体枚举代替。

***

### 解析而非校验 — `TryFrom` 与已验证类型

"解析，而非校验"是一个原则，它说的是：**不要先校验数据然后继续传递未校验的原始形式 — 而是将数据解析为一种只有在数据有效时才能存在的类型。** Rust 的 `TryFrom` trait 是实现这一点的标准工具。

#### 问题：没有强制力的校验

```rust
// ❌ 先校验后使用：没有任何东西阻止在校验之后使用无效值
fn process_port(port: u16) {
    if port == 0 || port > 65535 {
        panic!("Invalid port");           // 我们检查了，但是...
    }
    start_server(port);                    // 如果有人直接调用 start_server(0) 呢？
}

// ❌ 字符串类型：email 只是一个 String — 任何垃圾都能通过
fn send_email(to: String, body: String) {
    // `to` 真的是有效的 email 吗？我们不知道。
    // 有人可能传入 "not-an-email"，而我们只有在 SMTP 服务器才发现。
}
```

#### 解决方案：用 `TryFrom` 解析为已验证的新类型

```rust
use std::convert::TryFrom;
use std::fmt;

/// 一个已验证的 TCP 端口号（1–65535）。
/// 如果你拥有一个 `Port`，它就保证是有效的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port(u16);

impl TryFrom<u16> for Port {
    type Error = PortError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == 0 {
            Err(PortError::Zero)
        } else {
            Ok(Port(value))
        }
    }
}

impl Port {
    pub fn get(&self) -> u16 { self.0 }
}

#[derive(Debug)]
pub enum PortError {
    Zero,
    InvalidFormat,
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortError::Zero => write!(f, "port must be non-zero"),
            PortError::InvalidFormat => write!(f, "invalid port format"),
        }
    }
}

impl std::error::Error for PortError {}

// 现在类型系统强制保证了有效性：
fn start_server(port: Port) {
    // 无需校验 — Port 只能通过 TryFrom 构造，
    // 而它已经验证过有效性。
    println!("Listening on port {}", port.get());
}

// 用法：
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = Port::try_from(8080)?;   // ✅ 在边界处校验一次
    start_server(port);                  // 下游任何地方都不再重新校验

    let bad = Port::try_from(0);         // ❌ Err(PortError::Zero)
    Ok(())
}
```

#### 真实示例：已验证的 IPMI 地址

```rust
/// 一个已验证的 IPMI 从地址（0x20–0xFE，仅偶数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpmiAddr(u8);

#[derive(Debug)]
pub enum IpmiAddrError {
    Odd(u8),
    OutOfRange(u8),
}

impl fmt::Display for IpmiAddrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpmiAddrError::Odd(v) => write!(f, "IPMI address 0x{v:02X} must be even"),
            IpmiAddrError::OutOfRange(v) => {
                write!(f, "IPMI address 0x{v:02X} out of range (0x20..=0xFE)")
            }
        }
    }
}

impl TryFrom<u8> for IpmiAddr {
    type Error = IpmiAddrError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value % 2 != 0 {
            Err(IpmiAddrError::Odd(value))
        } else if value < 0x20 || value > 0xFE {
            Err(IpmiAddrError::OutOfRange(value))
        } else {
            Ok(IpmiAddr(value))
        }
    }
}

impl IpmiAddr {
    pub fn get(&self) -> u8 { self.0 }
}

// 下游代码永远不需要重新检查：
fn send_ipmi_command(addr: IpmiAddr, cmd: u8, data: &[u8]) -> Result<Vec<u8>, IpmiError> {
    // addr.get() 保证是一个有效的偶数 IPMI 地址
    raw_ipmi_send(addr.get(), cmd, data)
}
```

#### 使用 `FromStr` 解析字符串

对于通常从文本（CLI 参数、配置文件）解析的类型，请实现 `FromStr`：

```rust
use std::str::FromStr;

impl FromStr for Port {
    type Err = PortError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n: u16 = s.parse().map_err(|_| PortError::InvalidFormat)?;
        Port::try_from(n)
    }
}

// 现在可以用 .parse() 了：
let port: Port = "8080".parse()?;   // 一步完成校验

// 也能配合 clap CLI 解析使用：
// #[derive(Parser)]
// struct Args {
//     #[arg(short, long)]
//     port: Port,   // clap 自动调用 FromStr
// }
```

#### 复杂校验的 `TryFrom` 链

```rust
// 本示例的桩类型 — 在生产环境中，它们会位于
// 各自的模块中，拥有自己的 TryFrom 实现。
```

```rust
# struct Hostname(String);
# impl TryFrom<String> for Hostname {
#     type Error = String;
#     fn try_from(s: String) -> Result<Self, String> { Ok(Hostname(s)) }
# }
# struct Timeout(u64);
# impl TryFrom<u64> for Timeout {
#     type Error = String;
#     fn try_from(ms: u64) -> Result<Self, String> {
#         if ms == 0 { Err("timeout must be > 0".into()) } else { Ok(Timeout(ms)) }
#     }
# }
# struct RawConfig { host: String, port: u16, timeout_ms: u64 }
# #[derive(Debug)]
# enum ConfigError {
#     InvalidHost(String),
#     InvalidPort(PortError),
#     InvalidTimeout(String),
# }
# impl From<std::io::Error> for ConfigError {
#     fn from(e: std::io::Error) -> Self { ConfigError::InvalidHost(e.to_string()) }
# }
# impl From<serde_json::Error> for ConfigError {
#     fn from(e: serde_json::Error) -> Self { ConfigError::InvalidHost(e.to_string()) }
# }
/// 一个已验证的配置，只有所有字段都有效时才能存在。
pub struct ValidConfig {
    pub host: Hostname,
    pub port: Port,
    pub timeout_ms: Timeout,
}

impl TryFrom<RawConfig> for ValidConfig {
    type Error = ConfigError;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Ok(ValidConfig {
            host: Hostname::try_from(raw.host)
                .map_err(ConfigError::InvalidHost)?,
            port: Port::try_from(raw.port)
                .map_err(ConfigError::InvalidPort)?,
            timeout_ms: Timeout::try_from(raw.timeout_ms)
                .map_err(ConfigError::InvalidTimeout)?,
        })
    }
}

// 在边界处解析一次，然后在各处使用已验证的类型：
fn load_config(path: &str) -> Result<ValidConfig, ConfigError> {
    let raw: RawConfig = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    ValidConfig::try_from(raw)  // 所有校验都在这里发生
}
```

#### 小结：校验 vs 解析

| 方法 | 数据是否检查？ | 编译器是否强制有效性？ | 是否需要重新校验？ |
|----------|:---:|:---:|:---:|
| 运行时检查（if/assert） | ✅ | ❌ | 每个函数边界 |
| 已验证新类型 + `TryFrom` | ✅ | ✅ | 从不 — 类型即证明 |

规则：**在边界处解析，内部各处使用已验证的类型。**
原始字符串、整数和字节切片进入你的系统，通过 `TryFrom`/`FromStr` 被解析为已验证的类型，从那一刻起，类型系统保证它们是有效的。

### 特性开关与条件编译

```toml
# Cargo.toml
[features]
default = ["json"]          # 默认启用
json = ["dep:serde_json"]   # 启用 JSON 支持
xml = ["dep:quick-xml"]     # 启用 XML 支持
full = ["json", "xml"]      # 元特性：启用全部

[dependencies]
serde = "1"
serde_json = { version = "1", optional = true }
quick-xml = { version = "0.31", optional = true }
```

```rust
// 基于特性的条件编译：
#[cfg(feature = "json")]
pub fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

#[cfg(feature = "xml")]
pub fn to_xml<T: serde::Serialize>(value: &T) -> String {
    quick_xml::se::to_string(value).unwrap()
}

// 如果未启用所需特性，编译报错：
#[cfg(not(any(feature = "json", feature = "xml")))]
compile_error!("At least one format feature (json, xml) must be enabled");
```

**最佳实践**：
- 保持 `default` 特性最小化 — 用户可以按需启用
- 对可选依赖使用 `dep:` 语法（Rust 1.60+）以避免创建隐式特性
- 在 README 和 crate 级文档中记录特性

### 工作区组织

对于大型项目，使用 Cargo 工作区来共享依赖和构建产物：

```toml
# 根 Cargo.toml
[workspace]
members = [
    "core",         # 共享类型和 trait
    "parser",       # 解析库
    "server",       # 二进制 — 主应用
    "client",       # 客户端库
    "cli",          # CLI 二进制
]

# 共享依赖版本：
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

# 在每个成员的 Cargo.toml 中：
# [dependencies]
# serde = { workspace = true }
```

**好处**：

- 单一 `Cargo.lock` — 所有 crate 使用相同的依赖版本
- `cargo test --workspace` 运行所有测试
- 共享构建缓存 — 编译一个 crate 让所有 crate 受益
- 组件之间清晰的依赖边界

### `.cargo/config.toml`：项目级配置

`.cargo/config.toml` 文件（位于工作区根目录或 `$HOME/.cargo/` 中）无需修改 `Cargo.toml` 即可自定义 Cargo 行为：

```toml
# .cargo/config.toml

# 本工作区的默认目标
[build]
target = "x86_64-unknown-linux-gnu"

# 自定义运行器 — 例如，通过 QEMU 运行交叉编译的二进制
[target.aarch64-unknown-linux-gnu]
runner = "qemu-aarch64-static"
linker = "aarch64-linux-gnu-gcc"

# Cargo 别名 — 自定义快捷命令
[alias]
xt = "test --workspace --release"        # cargo xt = release 模式运行所有测试
ci = "clippy --workspace -- -D warnings" # cargo ci = 警告即报错的 lint
cov = "llvm-cov --workspace"             # cargo cov = 覆盖率（需要 cargo-llvm-cov）

# 构建脚本的环境变量
[env]
IPMI_LIB_PATH = "/usr/lib/bmc"

# 使用自定义注册表（用于内部包）
# [registries.internal]
# index = "https://gitlab.internal/crates/index"
```

常见配置模式：

| 设置 | 用途 | 示例 |
|---------|---------|---------|
| `[build] target` | 默认编译目标 | `x86_64-unknown-linux-musl` 用于静态构建 |
| `[target.X] runner` | 如何运行二进制 | `"qemu-aarch64-static"` 用于交叉编译 |
| `[target.X] linker` | 使用哪个链接器 | `"aarch64-linux-gnu-gcc"` |
| `[alias]` | 自定义 `cargo` 子命令 | `xt = "test --workspace"` |
| `[env]` | 构建时环境变量 | 库路径、特性开关 |
| `[net] offline` | 阻止网络访问 | `true` 用于隔离网络构建 |

### 编译期环境变量：`env!()` 和 `option_env!()`

Rust 可以在编译期将环境变量嵌入二进制文件中 — 对版本字符串、构建元数据和配置很有用：

```rust
// env!() — 如果变量缺失，编译期 panic
const VERSION: &str = env!("CARGO_PKG_VERSION"); // "0.1.0" 来自 Cargo.toml
const PKG_NAME: &str = env!("CARGO_PKG_NAME");   // 来自 Cargo.toml 的 crate 名

// option_env!() — 返回 Option<&str>，缺失时不 panic
const BUILD_SHA: Option<&str> = option_env!("GIT_SHA");
const BUILD_TIME: Option<&str> = option_env!("BUILD_TIMESTAMP");

fn print_version() {
    println!("{PKG_NAME} v{VERSION}");
    if let Some(sha) = BUILD_SHA {
        println!("  commit: {sha}");
    }
    if let Some(time) = BUILD_TIME {
        println!("  built:  {time}");
    }
}
```

Cargo 自动设置许多有用的环境变量：

| 变量 | 值 | 用例 |
|----------|-------|----------|
| `CARGO_PKG_VERSION` | `"1.2.3"` | 版本报告 |
| `CARGO_PKG_NAME` | `"diag_tool"` | 二进制识别 |
| `CARGO_PKG_AUTHORS` | 来自 `Cargo.toml` | 关于/帮助文本 |
| `CARGO_MANIFEST_DIR` | `Cargo.toml` 的绝对路径 | 定位测试数据文件 |
| `OUT_DIR` | 构建输出目录 | `build.rs` 代码生成目标 |
| `TARGET` | 目标三元组 | `build.rs` 中的平台特定逻辑 |

你可以从 `build.rs` 设置自定义环境变量：
```rust
// build.rs
fn main() {
    println!("cargo::rustc-env=GIT_SHA={}", git_sha());
    println!("cargo::rustc-env=BUILD_TIMESTAMP={}", timestamp());
}
```

### `cfg_attr`：条件属性

`cfg_attr` **仅当**条件为真时才应用属性。这比 `#[cfg()]`（包含/排除整个条目）更精细：

```rust
// 仅当 "serde" 特性启用时派生 Serialize：
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct DiagResult {
    pub fc: u32,
    pub passed: bool,
    pub message: String,
}
// 无 "serde" 特性：完全不需要 serde 依赖
// 有 "serde" 特性：DiagResult 可序列化

// 用于测试的条件属性：
#[cfg_attr(test, derive(PartialEq))]  // 仅在测试构建中派生 PartialEq
pub struct LargeStruct { /* ... */ }

// 平台特定的函数属性：
#[cfg_attr(target_os = "linux", link_name = "ioctl")]
#[cfg_attr(target_os = "freebsd", link_name = "__ioctl")]
extern "C" fn platform_ioctl(fd: i32, request: u64) -> i32;
```

| 模式 | 作用 |
|---------|-------------|
| `#[cfg(feature = "x")]` | 包含/排除整个条目 |
| `#[cfg_attr(feature = "x", derive(Foo))]` | 仅当特性 "x" 开启时添加 `derive(Foo)` |
| `#[cfg_attr(test, allow(unused))]` | 仅在测试构建中抑制警告 |
| `#[cfg_attr(doc, doc = "...")]` | 仅在 `cargo doc` 中可见的文档 |

### `cargo deny` 和 `cargo audit`：供应链安全

```bash
# 安装安全审计工具
cargo install cargo-deny
cargo install cargo-audit

# 检查依赖中的已知漏洞
cargo audit

# 全面检查：许可证、禁用、公告、来源
cargo deny check
```

使用工作区根目录的 `deny.toml` 配置 `cargo deny`：

```toml
# deny.toml
[advisories]
vulnerability = "deny"      # 发现已知漏洞则失败
unmaintained = "warn"        # 发现未维护 crate 则警告

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause"]
deny = ["GPL-3.0"]          # 拒绝 copyleft 许可证

[bans]
multiple-versions = "warn"  # 同一 crate 多版本时警告
deny = [
    { name = "openssl" },   # 强制使用 rustls 替代
]

[sources]
allow-git = []              # 生产环境不允许 git 依赖
```

| 工具 | 用途 | 何时运行 |
|------|---------|-------------|
| `cargo audit` | 检查依赖中的已知 CVE | CI 流水线、发布前 |
| `cargo deny check` | 许可证、禁用、公告、来源 | CI 流水线 |
| `cargo deny check licenses` | 仅许可证合规 | 开源前 |
| `cargo deny check bans` | 阻止特定 crate | 强制架构决策 |

### 文档测试：文档中的测试

Rust 文档注释（`///`）可以包含**被编译并作为测试运行**的代码块：

```rust
/// 从字符串解析诊断故障码。
///
/// # 示例
///
/// ```
/// use my_crate::parse_fc;
///
/// let fc = parse_fc("FC:12345").unwrap();
/// assert_eq!(fc, 12345);
/// ```
///
/// 无效输入返回错误：
///
/// ```
/// use my_crate::parse_fc;
///
/// assert!(parse_fc("not-a-fc").is_err());
/// ```
pub fn parse_fc(input: &str) -> Result<u32, ParseError> {
    input.strip_prefix("FC:")
        .ok_or(ParseError::MissingPrefix)?
        .parse()
        .map_err(ParseError::InvalidNumber)
}
```

```bash
cargo test --doc  # 仅运行文档测试
cargo test        # 运行单元 + 集成 + 文档测试
```

**模块级文档**使用文件顶部的 `//!`：

```rust
//! # 诊断框架
//!
//! 本 crate 提供核心诊断执行引擎。
//! 它支持运行诊断测试、收集结果，
//! 并通过 IPMI 向 BMC 报告。
//!
//! ## 快速开始
//!
//! ```no_run
//! use diag_framework::Framework;
//!
//! let mut fw = Framework::new("config.json")?;
//! fw.run_all_tests()?;
//! ```
```

### 使用 Criterion 进行基准测试

> **完整覆盖**：请参阅第 13 章（测试与基准测试模式）中的
> [使用 criterion 进行基准测试](ch14-testing-and-benchmarking-patterns.md#benchmarking-with-criterion)
> 一节，了解完整的 `criterion` 设置、API 示例以及与 `cargo bench` 的对比表。
> 下面是面向架构特定用法的快速参考。

对你的 crate 公共 API 进行基准测试时，将基准测试放在 `benches/` 目录，并聚焦于热路径 — 通常是解析器、序列化器或校验边界：

```bash
cargo bench                  # 运行所有基准测试
cargo bench -- parse_config  # 运行特定基准测试
# 结果在 target/criterion/ 中，附带 HTML 报告
```

> **关键要点 — 架构与 API 设计**
> - 接受最通用的类型（`impl Into`、`impl AsRef`、`Cow`）；返回最具体的类型
> - 解析而非校验：使用 `TryFrom` 创建构造即有效的类型
> - 公共枚举上的 `#[non_exhaustive]` 防止添加变体时的破坏性变更
> - `#[must_use]` 捕获对重要值的静默丢弃

> **另见：** [第 9 章 — 错误处理](ch10-error-handling-patterns.md) 了解公共 API 中的错误类型设计。[第 13 章 — 测试](ch14-testing-and-benchmarking-patterns.md) 了解如何测试你的 crate 公共 API。

---

### 练习：Crate API 重构 ★★（约 30 分钟）

将以下"字符串类型"的 API 重构为使用 `TryFrom`、新类型模式和建造者模式的版本：

```rust,ignore
// 改造前：容易误用
fn create_server(host: &str, port: &str, max_conn: &str) -> Server { ... }
```

设计一个 `ServerConfig`，使用已验证的类型 `Host`、`Port`（1–65535）和 `MaxConnections`（1–10000），在解析时拒绝无效值。

<details>
<summary>🔑 解答</summary>

```rust
#[derive(Debug, Clone)]
struct Host(String);

impl TryFrom<&str> for Host {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, String> {
        if s.is_empty() { return Err("host cannot be empty".into()); }
        if s.contains(' ') { return Err("host cannot contain spaces".into()); }
        Ok(Host(s.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
struct Port(u16);

impl TryFrom<u16> for Port {
    type Error = String;
    fn try_from(p: u16) -> Result<Self, String> {
        if p == 0 { return Err("port must be >= 1".into()); }
        Ok(Port(p))
    }
}

#[derive(Debug, Clone, Copy)]
struct MaxConnections(u32);

impl TryFrom<u32> for MaxConnections {
    type Error = String;
    fn try_from(n: u32) -> Result<Self, String> {
        if n == 0 || n > 10_000 {
            return Err(format!("max_connections must be 1–10000, got {n}"));
        }
        Ok(MaxConnections(n))
    }
}

#[derive(Debug)]
struct ServerConfig {
    host: Host,
    port: Port,
    max_connections: MaxConnections,
}

impl ServerConfig {
    fn new(host: Host, port: Port, max_connections: MaxConnections) -> Self {
        ServerConfig { host, port, max_connections }
    }
}

fn main() {
    let config = ServerConfig::new(
        Host::try_from("localhost").unwrap(),
        Port::try_from(8080).unwrap(),
        MaxConnections::try_from(100).unwrap(),
    );
    println!("{config:?}");

    // 无效值在解析时被捕获：
    assert!(Host::try_from("").is_err());
    assert!(Port::try_from(0).is_err());
    assert!(MaxConnections::try_from(99999).is_err());
}
```

</details>

***
