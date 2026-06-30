# 14. 测试与基准测试模式 🟢

> **你将学到：**
> - Rust 的三个测试层级：单元测试、集成测试和文档测试
> - 使用 proptest 进行基于属性的测试以发现边界情况
> - 使用 criterion 进行可靠的性能基准测试
> - 无需重量级框架的模拟（mocking）策略

## 单元测试、集成测试、文档测试

Rust 内置了三个测试层级：

```rust
// === 单元测试：与被测代码放在同一文件中 ===
// 单元测试关注模块内部逻辑，可访问私有项（通过 use super::*）。
pub fn factorial(n: u64) -> u64 {
    // → Iterator::product：对范围迭代器做乘积累积。
    //   签名：fn product<Self>(self) -> Self::Item where Self: Iterator + Sized
    //   (1..=n) 是 RangeInclusive<u64>，其 Item = u64，故返回 u64。
    //   空范围（1..=0）的 product 返回 1（乘法单位元）。
    (1..=n).product()
}

// → #[cfg(test)]：条件编译属性，使 tests 模块仅在 `cargo test` 时编译，
//   正常构建中不会产生任何代码体积开销。
#[cfg(test)]
mod tests {
    // → use super::*：导入父模块（被测模块）的所有公有与私有项，
    //   使测试函数能直接访问 factorial 等内部实现。
    use super::*;

    // → #[test]：属性宏，将该函数标记为测试用例。
    //   cargo test 会发现并运行所有带 #[test] 的函数；函数返回 () 或 Result。
    #[test]
    fn test_factorial_zero() {
        // (1..=0).product() 返回 1 —— 空范围的乘法单位元
        // → assert_eq!(left, right)：断言两值相等（PartialEq），
        //   不等时 panic 并打印两边值（需 Debug）。这是最常用的断言宏。
        assert_eq!(factorial(0), 1);
    }

    #[test]
    fn test_factorial_five() {
        assert_eq!(factorial(5), 120);
    }

    #[test]
    // → #[cfg(debug_assertions)]：仅在 debug 构建（dev profile）启用此测试，
    //   debug_assertions 配置标志默认在非优化构建中开启。
    #[cfg(debug_assertions)] // 溢出检查仅在 debug 模式启用
    // → #[should_panic(expected = "...")]：属性，断言该测试**必须 panic**，
    //   且 panic 消息应包含 "overflow"。若不 panic 则测试失败。
    #[should_panic(expected = "overflow")]
    fn test_factorial_overflow() {
        // ⚠️ 此测试仅在 debug 模式（启用溢出检查）通过。
        // 在 release 模式（`cargo test --release`）下，u64 算术静默回绕，
        // 不会触发 panic。使用 `checked_mul` 或在 profile 中设置
        // `overflow-checks = true` 以获得 release 模式的安全保证。
        factorial(100); // 应当因溢出而 panic
    }

    #[test]
    // → 测试函数可返回 Result<(), E>，使 ? 运算符在测试中可用。
    //   返回 Err 时测试失败；返回 Ok(()) 时通过。Box<dyn Error> 接受任何错误。
    fn test_with_result() -> Result<(), Box<dyn std::error::Error>> {
        // 测试可以返回 Result —— ? 在内部可用！
        // → str::parse::<F>()：将字符串解析为目标类型 F: FromStr，
        //   返回 Result<F, F::Err>。这里 F = u64。
        let value: u64 = "42".parse()?;
        assert_eq!(value, 42);
        Ok(())
    }
}
```

```rust
// --- 集成测试：位于 tests/ 目录 ---
// tests/integration_test.rs
// 这些测试**只**能访问 crate 的公共 API
// → 集成测试以独立 crate 形式存在，只能 import 被测 crate 的 pub 项，
//   用于验证对外契约，而非内部实现细节。

use my_crate::factorial;

#[test]
fn test_factorial_from_outside() {
    assert_eq!(factorial(10), 3_628_800);
}
```

```rust
// --- 文档测试：位于文档注释中 ---
// → 文档注释 /// 是 Markdown，其中的 ```rust 代码块会被 rustdoc 提取、
//   编译并作为测试运行（cargo test 会自动包含文档测试）。
/// Computes the factorial of `n`.
///
/// # Examples
///
/// ```
/// use my_crate::factorial;
/// assert_eq!(factorial(5), 120);
/// ```
///
/// # Panics
///
/// Panics if the result overflows `u64`.
///
/// ```should_panic
/// my_crate::factorial(100);
/// ```
pub fn factorial(n: u64) -> u64 {
    (1..=n).product()
}
// 文档测试由 `cargo test` 编译并运行 —— 它们保证示例始终与代码同步。
// → 代码块标注 ```should_panic 表示此示例**必须** panic 才算通过，
//   用于演示会触发 panic 的边界情况。
```

### 测试夹具与初始化

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 共享初始化 —— 创建一个辅助函数
    fn setup_database() -> TestDb {
        let db = TestDb::new_in_memory();
        db.run_migrations();
        db.seed_test_data();
        db
    }

    #[test]
    fn test_user_creation() {
        // → 在测试开始处调用 setup_database() 复用初始化逻辑，
        //   替代每个测试重复编写夹具代码。
        let db = setup_database();
        // → Result::unwrap()：解包 Result，若为 Err 则 panic（测试中常用）。
        let user = db.create_user("Alice", "alice@test.com").unwrap();
        assert_eq!(user.name, "Alice");
    }

    #[test]
    fn test_user_deletion() {
        let db = setup_database();
        db.create_user("Bob", "bob@test.com").unwrap();
        // → Option::is_some / Result::is_ok：返回 bool，用于 assert! 断言。
        assert!(db.delete_user("Bob").is_ok());
        assert!(db.get_user("Bob").is_none());
    }

    // 使用 Drop（RAII）做清理：
    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            // Cargo.toml: rand = "0.8"
            // → std::env::temp_dir()：返回系统临时目录（OS 相关）。
            //   PathBuf::join 拼接路径，rand::random::<u32>() 生成随机数防冲突。
            let path = std::env::temp_dir().join(format!("test_{}", rand::random::<u32>()));
            // → std::fs::create_dir_all：递归创建目录（含父目录），返回 io::Result<()>。
            std::fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }
    }

    // → impl Drop for T：RAII 守卫，T 离开作用域时自动调用 drop(&mut self)，
    //   无需手动清理 —— 这是 Rust 资源管理的核心机制。
    impl Drop for TempDir {
        fn drop(&mut self) {
            // → std::fs::remove_dir_all：递归删除目录树，忽略错误（let _ =）。
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_file_operations() {
        let dir = TempDir::new(); // 创建临时目录
        std::fs::write(dir.path.join("test.txt"), "hello").unwrap();
        assert!(dir.path.join("test.txt").exists());
    } // dir 在此 drop → 临时目录被自动清理
}
```

### 基于属性的测试（proptest）

与其测试特定值，不如测试应该始终成立的*属性*：

```rust
// Cargo.toml: proptest = "1"
// → proptest：基于属性的测试框架。它随机生成大量输入并验证某个"属性"
//   （不变量）始终成立；发现失败时自动将用例缩减为最小复现输入。
// → use proptest::prelude::*：导入常用策略（Strategy）、proptest! 宏、prop_assert! 等。
use proptest::prelude::*;

fn reverse(v: &[i32]) -> Vec<i32> {
    // → iter().rev().cloned().collect()：反向遍历引用，克隆出值，收集成 Vec。
    v.iter().rev().cloned().collect()
}

// → proptest! { ... }：宏，包裹若干基于属性的测试函数。
//   每个测试通过 "name(args in strategy)" 声明输入策略。
proptest! {
    #[test]
    // → prop::collection::vec(strategy, range)：生成任意长度（0..100）的 Vec，
    //   元素由 any::<i32>() 策略生成（任意 i32，含边界值）。
    fn test_reverse_twice_is_identity(v in prop::collection::vec(any::<i32>(), 0..100)) {
        // 属性：反转两次应得到原向量
        assert_eq!(reverse(&reverse(&v)), v);
    }

    #[test]
    fn test_reverse_preserves_length(v in prop::collection::vec(any::<i32>(), 0..100)) {
        assert_eq!(reverse(&v).len(), v.len());
    }

    #[test]
    fn test_sort_is_idempotent(mut v in prop::collection::vec(any::<i32>(), 0..100)) {
        // → Vec::sort：原地排序（mut 借用 self）。
        v.sort();
        let sorted_once = v.clone();
        v.sort();
        assert_eq!(v, sorted_once); // 排序两次 = 排序一次（幂等性）
    }

    #[test]
    // → Strategy::prop_filter：对策略产生的值施加过滤条件，仅保留 is_finite() 的 f64
    //   （排除 NaN/Infinity），并用描述信息记录过滤原因。
    fn test_parse_roundtrip(x in any::<f64>().prop_filter("finite", |x| x.is_finite())) {
        // 属性：格式化后再解析应得到原值
        let s = format!("{x}");
        // → prop_assert!：proptest 版 assert!，失败时记录当前随机种子以便复现。
        let parsed: f64 = s.parse().unwrap();
        prop_assert!((x - parsed).abs() < f64::EPSILON);
    }
}
```

> **何时使用 proptest**：当你测试一个输入空间很大的函数，并且希望确信它对你
> 没想到的边界情况也能正常工作时。proptest 会生成数百个随机输入，并将失败案例
> 缩减到最小重现用例。

### 使用 criterion 进行基准测试

```rust
// Cargo.toml:
// [dev-dependencies]
// criterion = { version = "0.5", features = ["html_reports"] }
//
// [[bench]]
// name = "my_benchmarks"
// harness = false   // → 禁用默认的 libtest 测试入口，
//                   //   改用 criterion 自带的 main 入口。

// benches/my_benchmarks.rs
// → criterion：统计上严谨的基准测试库，自动多次采样、计算置信区间，
//   并生成 HTML 报告对比历史结果，避免因噪声导致的误判。
use criterion::{criterion_group, criterion_main, Criterion, black_box};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 | 1 => n,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

// → bench 函数签名：fn(&mut Criterion)。criterion 会向其传入基准上下文 c。
fn bench_fibonacci(c: &mut Criterion) {
    // → Criterion::bench_function(id, routine)：创建一个基准测试，
    //   id 是测试名，routine 闭包接收 Bencher b。
    c.bench_function("fibonacci 20", |b| {
        // → Bencher::iter：测量闭包每次执行的耗时。
        //   black_box：阻止编译器优化掉看似"未使用"的计算结果，
        //   确保测量的是真实计算开销。
        b.iter(|| fibonacci(black_box(20)))
    });

    // 对比不同实现：
    // → Criterion::benchmark_group：创建一组基准测试，便于在报告中横向对比。
    let mut group = c.benchmark_group("fibonacci_compare");
    for size in [10, 15, 20, 25] {
        // → group.bench_with_input(id, input, routine)：
        //   以参数化输入运行基准，id 由 BenchmarkId::from_parameter 构造，
        //   使每个 size 在报告中独立显示。
        group.bench_with_input(
            criterion::BenchmarkId::from_parameter(size),
            &size,
            |b, &size| b.iter(|| fibonacci(black_box(size))),
        );
    }
    // → BenchmarkGroup::finish：完成本组测试，写入报告。必须调用。
    group.finish();
}

// → criterion_group!：声明性宏，将若干 bench 函数聚合为一个 group 函数。
criterion_group!(benches, bench_fibonacci);
// → criterion_main!：生成 main() 入口，运行给定 group。
criterion_main!(benches);

// 运行：cargo bench
// 结果在 target/criterion/ 中，附带 HTML 报告
```

### 无需框架的模拟策略

Rust 的 trait 系统提供了天然的依赖注入——不需要模拟框架：

```rust
// 将行为定义为 trait
// → trait 定义一组方法契约；生产实现与测试替身都实现它，实现依赖注入。
trait Clock {
    // → std::time::Instant：单调递增的时间点（用于测量经过时间，非墙上时钟）。
    fn now(&self) -> std::time::Instant;
}

trait HttpClient {
    fn get(&self, url: &str) -> Result<String, String>;
}

// 生产实现
struct RealClock;
impl Clock for RealClock {
    fn now(&self) -> std::time::Instant { std::time::Instant::now() }
}

// 服务依赖抽象而非具体类型
// → 泛型参数 <C: Clock, H: HttpClient>：约束 C 必须实现 Clock，H 实现 HttpClient。
//   这样 CacheService 可接受任何实现这两个 trait 的类型（含模拟实现）。
struct CacheService<C: Clock, H: HttpClient> {
    clock: C,
    client: H,
    ttl: std::time::Duration,
}

// → impl<C: Clock, H: HttpClient> CacheService<C, H>：
//   为所有满足约束的 C、H 实现 CacheService 的方法。
impl<C: Clock, H: HttpClient> CacheService<C, H> {
    fn fetch(&self, url: &str) -> Result<String, String> {
        // 使用 self.clock 和 self.client —— 可注入
        self.client.get(url)
    }
}

// 用模拟实现测试 —— 无需任何框架！
#[cfg(test)]
mod tests {
    use super::*;

    // 模拟时钟：返回固定的预设时间，使测试结果可复现。
    struct MockClock {
        fixed_time: std::time::Instant,
    }
    impl Clock for MockClock {
        fn now(&self) -> std::time::Instant { self.fixed_time }
    }

    // 模拟 HTTP 客户端：总是返回预设响应，不发起真实网络请求。
    struct MockHttpClient {
        response: String,
    }
    impl HttpClient for MockHttpClient {
        fn get(&self, _url: &str) -> Result<String, String> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn test_cache_service() {
        // → 注入 MockClock 与 MockHttpClient，验证 CacheService 逻辑，
        //   而无需真实时钟或网络依赖。
        let service = CacheService {
            clock: MockClock { fixed_time: std::time::Instant::now() },
            client: MockHttpClient { response: "cached data".into() },
            ttl: std::time::Duration::from_secs(300),
        };

        assert_eq!(service.fetch("http://example.com").unwrap(), "cached data");
    }
}
```

> **测试哲学**：在集成测试中优先使用真实依赖，在单元测试中使用基于 trait 的模拟。
> 除非你的依赖图很复杂，否则避免使用模拟框架——Rust 的 trait 泛型能自然处理大多数情况。

> **关键要点——测试**
> - 文档测试（`///`）兼作文档和回归测试——它们会被编译并运行
> - `proptest` 生成随机输入来发现你永远不会手动编写的边界情况
> - `criterion` 提供统计上严谨的基准测试并附带 HTML 报告
> - 通过 trait 泛型 + 测试替身（test double）进行模拟，而非使用模拟框架

> **另请参阅：**[第 12 章——宏](ch13-macros-code-that-writes-code.md)了解如何测试宏生成的代码。[第 14 章——API 设计](ch15-crate-architecture-and-api-design.md)了解模块布局如何影响测试组织。

---

### 练习：使用 proptest 进行基于属性的测试 ★★（约 25 分钟）

编写一个 `SortedVec<T: Ord>` 包装类型，维护已排序的不变量。使用 `proptest` 验证：
1. 在任意插入序列之后，内部 vec 始终是已排序的
2. `contains()` 与标准库的 `Vec::contains()` 结果一致
3. 长度等于插入次数

<details>
<summary>🔑 解答</summary>

```rust,ignore
#[derive(Debug)]
// → 新类型 SortedVec<T: Ord>：包装 Vec 并维护"始终有序"的不变量。
//   T: Ord 约束要求元素可全序比较（这是二分查找的前提）。
struct SortedVec<T: Ord> {
    inner: Vec<T>,
}

impl<T: Ord> SortedVec<T> {
    fn new() -> Self { SortedVec { inner: Vec::new() } }

    // → insert 通过二分查找定位插入位置，保持有序（O(n) 含移动）。
    fn insert(&mut self, value: T) {
        // → slice::binary_search：在已排序切片中二分查找。
        //   返回 Result<usize, usize>：Ok(pos) 表示找到；Err(pos) 表示应插入处。
        //   unwrap_or_else(|p| p) 将 Err(pos) 转为插入下标。
        let pos = self.inner.binary_search(&value).unwrap_or_else(|p| p);
        // → Vec::insert(index, value)：在 index 处插入，后移后续元素。
        self.inner.insert(pos, value);
    }

    fn contains(&self, value: &T) -> bool {
        // → Result::is_ok：返回 true 当且仅当 binary_search 找到目标。
        self.inner.binary_search(value).is_ok()
    }

    fn len(&self) -> usize { self.inner.len() }
    fn as_slice(&self) -> &[T] { &self.inner }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        // → 策略 -1000i32..1000：限定范围的整数策略，缩小输入空间便于复现。
        fn always_sorted(values in proptest::collection::vec(-1000i32..1000, 0..100)) {
            let mut sv = SortedVec::new();
            for v in &values {
                sv.insert(*v);
            }
            // → slice::windows(2)：返回所有相邻元素对的滑动窗口，
            //   用于验证每一对 w[0] <= w[1]（单调不减）。
            for w in sv.as_slice().windows(2) {
                prop_assert!(w[0] <= w[1]);
            }
            prop_assert_eq!(sv.len(), values.len());
        }

        #[test]
        fn contains_matches_stdlib(values in proptest::collection::vec(0i32..50, 1..30)) {
            let mut sv = SortedVec::new();
            for v in &values {
                sv.insert(*v);
            }
            for v in &values {
                prop_assert!(sv.contains(v));
            }
            // → prop_assert!(!...)：断言某元素不在集合中（边界外的哨兵值）。
            prop_assert!(!sv.contains(&9999));
        }
    }
}
```

</details>

***
