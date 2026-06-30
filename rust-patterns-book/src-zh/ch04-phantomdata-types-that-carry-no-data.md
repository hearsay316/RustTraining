# 4. PhantomData——不携带数据的类型 🔴

> **你将学到：**
> - 为什么 `PhantomData<T>` 会存在，以及它解决的三个问题
> - 用于编译期作用域强制的生命周期标记（lifetime branding）
> - 用于维度安全运算的计量单位模式（unit-of-measure pattern）
> - 变型（协变 covariance、逆变 contravariance、不变 invariant）以及 PhantomData 如何控制它

## PhantomData 解决了什么问题

`PhantomData<T>` 是一个零大小类型，它告诉编译器"这个结构体在逻辑上与 `T` 相关联，尽管它并不包含一个 `T`"。它会影响变型、drop 检查以及 auto trait 的推断——而且不占用任何内存。

```rust
use std::marker::PhantomData;

// 没有 PhantomData 时：
struct Slice<'a, T> {
    ptr: *const T,
    len: usize,
    // 问题：编译器不知道这个结构体从 'a 借用
    // 也不知道它与 T 关联（用于 drop 检查）
}

// 有 PhantomData 时：
struct Slice<'a, T> {
    ptr: *const T,
    len: usize,
    _marker: PhantomData<&'a T>,
    // 现在编译器知道：
    // 1. 这个结构体以生命周期 'a 借用数据
    // 2. 它对 'a 协变（生命周期可以缩短）
    // 3. drop 检查会考虑 T
}
```

**PhantomData 的三大职责**：

| 职责 | 示例 | 作用 |
|-----|---------|-------------|
| **生命周期绑定** | `PhantomData<&'a T>` | 结构体被视为借用了 `'a` |
| **所有权模拟** | `PhantomData<T>` | drop 检查假定结构体拥有一个 `T` |
| **变型控制** | `PhantomData<fn(T)>` | 使结构体对 `T` 逆变 |

### 生命周期标记

使用 `PhantomData` 来防止不同"会话"或"上下文"中的值被混用：

```rust
use std::cell::RefCell;
use std::marker::PhantomData;

/// 一个仅在特定 arena 生命周期内有效的句柄
struct ArenaHandle<'arena> {
    index: usize,
    _brand: PhantomData<&'arena ()>,
}

struct Arena {
    data: RefCell<Vec<String>>,
}

impl Arena {
    fn new() -> Self {
        Arena { data: RefCell::new(Vec::new()) }
    }

    /// 分配一个字符串并返回带标记的句柄
    fn alloc(&self, value: String) -> ArenaHandle<'_> {
        let mut data = self.data.borrow_mut();
        let index = data.len();
        data.push(value);
        ArenaHandle { index, _brand: PhantomData }
    }

    /// 通过句柄查找——只接受来自本 arena 的句柄
    fn get<'a>(&'a self, handle: ArenaHandle<'a>) -> String {
        let data = self.data.borrow();
        data[handle.index].clone()
    }
}

fn main() {
    let arena1 = Arena::new();
    let handle1 = arena1.alloc("hello".to_string());

    // 不能将 handle1 用于不同的 arena——生命周期不匹配
    // let arena2 = Arena::new();
    // arena2.get(handle1); // ❌ 生命周期不匹配

    println!("{}", arena1.get(handle1)); // ✅
}
```

### 计量单位模式

在编译期防止不兼容单位的混用，且零运行时开销：

```rust
use std::marker::PhantomData;
use std::ops::{Add, Mul};

// 单位标记类型（零大小）
struct Meters;
struct Seconds;
struct MetersPerSecond;

#[derive(Debug, Clone, Copy)]
struct Quantity<Unit> {
    value: f64,
    _unit: PhantomData<Unit>,
}

impl<U> Quantity<U> {
    fn new(value: f64) -> Self {
        Quantity { value, _unit: PhantomData }
    }
}

// 只能添加相同单位：
impl<U> Add for Quantity<U> {
    type Output = Quantity<U>;
    fn add(self, rhs: Self) -> Self::Output {
        Quantity::new(self.value + rhs.value)
    }
}

// Meters / Seconds = MetersPerSecond（自定义 trait）
impl std::ops::Div<Quantity<Seconds>> for Quantity<Meters> {
    type Output = Quantity<MetersPerSecond>;
    fn div(self, rhs: Quantity<Seconds>) -> Quantity<MetersPerSecond> {
        Quantity::new(self.value / rhs.value)
    }
}

fn main() {
    let dist = Quantity::<Meters>::new(100.0);
    let time = Quantity::<Seconds>::new(9.58);
    let speed = dist / time; // Quantity<MetersPerSecond>
    println!("Speed: {:.2} m/s", speed.value); // 10.44 m/s

    // let nonsense = dist + time; // ❌ 编译错误：不能把 Meters + Seconds 相加
}
```

> **这纯粹是类型系统的魔法**——`PhantomData<Meters>` 是零大小的，
> 因此 `Quantity<Meters>` 的内存布局与 `f64` 相同。运行时没有任何包装开销，
> 但编译期却拥有完整的单位安全性。

### PhantomData 与 Drop 检查

当编译器检查一个结构体的析构函数是否会访问已失效的数据时，它会使用 `PhantomData` 来做决策：

```rust
use std::marker::PhantomData;

// PhantomData<T> —— 编译器假设我们可能会 drop 一个 T
// 这意味着 T 必须比我们的结构体活得久
struct OwningSemantic<T> {
    ptr: *const T,
    _marker: PhantomData<T>,  // "我在逻辑上拥有一个 T"
}

// PhantomData<*const T> —— 编译器假设我们不拥有 T
// 更宽松——T 不需要比我们活得久
struct NonOwningSemantic<T> {
    ptr: *const T,
    _marker: PhantomData<*const T>,  // "我只是指向 T"
}
```

**实用规则**：在包装裸指针时，请谨慎选择 PhantomData：
- 编写一个拥有数据的容器？ → `PhantomData<T>`
- 编写一个视图/引用类型？ → `PhantomData<&'a T>` 或 `PhantomData<*const T>`

### 变型——为什么 PhantomData 的类型参数很重要

**变型（variance）**决定了一个泛型类型能否被替换为子类型或父类型（在 Rust 中，"子类型"意味着"具有更长的生命周期"）。变型搞错会导致要么好的代码被拒绝，要么不健全的代码被接受。

```mermaid
graph LR
    subgraph 协变
        direction TB
        A1["&'long T"] -->|"可以变成"| A2["&'short T"]
    end

    subgraph 逆变
        direction TB
        B1["fn(&'short T)"] -->|"可以变成"| B2["fn(&'long T)"]
    end

    subgraph 不变
        direction TB
        C1["&'a mut T"] ---|"不可替换"| C2["&'b mut T"]
    end

    style A1 fill:#d4efdf,stroke:#27ae60,color:#000
    style A2 fill:#d4efdf,stroke:#27ae60,color:#000
    style B1 fill:#e8daef,stroke:#8e44ad,color:#000
    style B2 fill:#e8daef,stroke:#8e44ad,color:#000
    style C1 fill:#fadbd8,stroke:#e74c3c,color:#000
    style C2 fill:#fadbd8,stroke:#e74c3c,color:#000
```

#### 三种变型

| 变型 | 含义 | "我可以替换……" | Rust 示例 |
|----------|---------|---------------------|--------------|
| **协变** | 子类型正向传递 | 期望 `'short` 处用 `'long` ✅ | `&'a T`、`Vec<T>`、`Box<T>` |
| **逆变** | 子类型反向传递 | 期望 `'long` 处用 `'short` ✅ | `fn(T)`（参数位置） |
| **不变** | 不允许替换 | 两个方向都不行 ✅ | `&mut T`、`Cell<T>`、`UnsafeCell<T>` |

#### 为什么 `&'a T` 对 `'a` 协变

```rust
fn print_str(s: &str) {
    println!("{s}");
}

fn main() {
    let owned = String::from("hello");
    // owned 存活整个函数周期（'long）
    // print_str 期望 &'_ str（'short —— 仅用于调用）
    print_str(&owned); // ✅ 协变：'long → 'short 是安全的
    // 更长的引用总是可以用于需要更短引用的地方。
}
```

#### 为什么 `&mut T` 对 `T` 不变

```rust
// 如果 &mut T 对 T 协变，这段代码就能编译：
fn evil(s: &mut &'static str) {
    // 我们可以把一个更短生命周期的 &str 写入 &'static str 的位置！
    let local = String::from("temporary");
    // *s = &local; // ← 会创建悬垂的 &'static str
}

// 不变性阻止了这种情况：可变时 &'static str ≠ &'a str。
// 编译器完全拒绝这种替换。
```

#### PhantomData 如何控制变型

`PhantomData<X>` 赋予你的结构体**与 `X` 相同的变型**：

```rust
use std::marker::PhantomData;

// 对 'a 协变——Ref<'long> 可以用作 Ref<'short>
struct Ref<'a, T> {
    ptr: *const T,
    _marker: PhantomData<&'a T>,  // 对 'a 协变，对 T 协变
}

// 对 T 不变——防止 T 的生命周期被不健全地缩短
struct MutRef<'a, T> {
    ptr: *mut T,
    _marker: PhantomData<&'a mut T>,  // 对 'a 协变，对 T 不变
}

// 对 T 逆变——适用于回调容器
struct CallbackSlot<T> {
    _marker: PhantomData<fn(T)>,  // 对 T 逆变
}
```

**PhantomData 变型速查表**：

| PhantomData 类型 | 对 `T` 的变型 | 对 `'a` 的变型 | 使用场景 |
|------------------|--------------------|--------------------|-----------|
| `PhantomData<T>` | 协变 | — | 你在逻辑上拥有一个 `T` |
| `PhantomData<&'a T>` | 协变 | 协变 | 你以生命周期 `'a` 借用一个 `T` |
| `PhantomData<&'a mut T>` | **不变** | 协变 | 你可变地借用 `T` |
| `PhantomData<*const T>` | 协变 | — | 指向 `T` 的非拥有型指针 |
| `PhantomData<*mut T>` | **不变** | — | 非拥有型可变指针 |
| `PhantomData<fn(T)>` | **逆变** | — | `T` 出现在参数位置 |
| `PhantomData<fn() -> T>` | 协变 | — | `T` 出现在返回位置 |
| `PhantomData<fn(T) -> T>` | **不变** | — | `T` 同时出现在两个位置会相互抵消 |

#### 实战示例：为什么这很重要

```rust
use std::marker::PhantomData;

// 一个用会话生命周期标记值的令牌。
// 必须对 'a 协变——否则调用方无法缩短
// 将其传递给需要更短借用的函数时的生命周期。
struct SessionToken<'a> {
    id: u64,
    _brand: PhantomData<&'a ()>,  // ✅ 协变——调用方可以缩短 'a
    // _brand: PhantomData<fn(&'a ())>,  // ❌ 逆变——破坏人体工程学
    // _brand: PhantomData<&'a mut ()>;  // 对 'a 仍然协变（对 T 不变，但 T 固定为 ()）
}

fn use_token(token: &SessionToken<'_>) {
    println!("Using token {}", token.id);
}

fn main() {
    let token = SessionToken { id: 42, _brand: PhantomData };
    use_token(&token); // ✅ 有效，因为 SessionToken 对 'a 协变
}
```

> **决策规则**：从 `PhantomData<&'a T>`（协变）开始。只有当你的抽象
> 向外提供对 `T` 的可变访问时，才切换为 `PhantomData<&'a mut T>`（不变）。
> 几乎不要使用 `PhantomData<fn(T)>`（逆变）——它仅对回调存储场景是正确的。

> **要点总结——PhantomData**
> - `PhantomData<T>` 携带类型/生命周期信息，且无运行时开销
> - 用于生命周期标记、变型控制以及计量单位模式
> - drop 检查：`PhantomData<T>` 告诉编译器你的类型在逻辑上拥有一个 `T`

> **另请参阅：**[第 3 章——Newtype 与类型状态](ch03-the-newtype-and-type-state-patterns.md)，了解使用 PhantomData 的类型状态模式。[第 11 章——Unsafe Rust](ch12-unsafe-rust-controlled-danger.md)，了解 PhantomData 如何与裸指针交互。

---

### 练习：基于 PhantomData 的计量单位 ★★（约 30 分钟）

扩展计量单位模式以支持：
- `Meters`、`Seconds`、`Kilograms`
- 相同单位的加法
- 乘法：`Meters * Meters = SquareMeters`
- 除法：`Meters / Seconds = MetersPerSecond`

<details>
<summary>🔑 答案</summary>

```rust
use std::marker::PhantomData;
use std::ops::{Add, Mul, Div};

#[derive(Clone, Copy)]
struct Meters;
#[derive(Clone, Copy)]
struct Seconds;
#[derive(Clone, Copy)]
struct Kilograms;
#[derive(Clone, Copy)]
struct SquareMeters;
#[derive(Clone, Copy)]
struct MetersPerSecond;

#[derive(Debug, Clone, Copy)]
struct Qty<U> {
    value: f64,
    _unit: PhantomData<U>,
}

impl<U> Qty<U> {
    fn new(v: f64) -> Self { Qty { value: v, _unit: PhantomData } }
}

impl<U> Add for Qty<U> {
    type Output = Qty<U>;
    fn add(self, rhs: Self) -> Self::Output { Qty::new(self.value + rhs.value) }
}

impl Mul<Qty<Meters>> for Qty<Meters> {
    type Output = Qty<SquareMeters>;
    fn mul(self, rhs: Qty<Meters>) -> Qty<SquareMeters> {
        Qty::new(self.value * rhs.value)
    }
}

impl Div<Qty<Seconds>> for Qty<Meters> {
    type Output = Qty<MetersPerSecond>;
    fn div(self, rhs: Qty<Seconds>) -> Qty<MetersPerSecond> {
        Qty::new(self.value / rhs.value)
    }
}

fn main() {
    let width = Qty::<Meters>::new(5.0);
    let height = Qty::<Meters>::new(3.0);
    let area = width * height; // Qty<SquareMeters>
    println!("Area: {:.1} m²", area.value);

    let dist = Qty::<Meters>::new(100.0);
    let time = Qty::<Seconds>::new(9.58);
    let speed = dist / time;
    println!("Speed: {:.2} m/s", speed.value);

    let sum = width + height; // 相同单位 ✅
    println!("Sum: {:.1} m", sum.value);

    // let bad = width + time; // ❌ 编译错误：不能把 Meters + Seconds 相加
}
```

</details>

***
