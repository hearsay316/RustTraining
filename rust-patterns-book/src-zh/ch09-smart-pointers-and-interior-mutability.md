# 9. 智能指针与内部可变性 🟡

> **你将学到：**
> - Box、Rc、Arc 用于堆分配和共享所有权
> - Weak 引用用于打破 Rc/Arc 的引用循环
> - Cell、RefCell 和 Cow 实现内部可变性模式
> - Pin 用于自引用类型，ManuallyDrop 用于生命周期控制

## Box、Rc、Arc — 堆分配与共享

```rust
// --- Box<T>：单一所有者，堆分配 ---
// 使用场景：递归类型、大尺寸值、trait 对象
let boxed: Box<i32> = Box::new(42);
println!("{}", *boxed); // 解引用为 i32

// 递归类型需要 Box（否则尺寸无限大）：
enum List<T> {
    Cons(T, Box<List<T>>),
    Nil,
}

// trait 对象（动态派发）：
let writer: Box<dyn std::io::Write> = Box::new(std::io::stdout());

// --- Rc<T>：多个所有者，单线程 ---
// 使用场景：单线程内的共享所有权（非 Send/Sync）
use std::rc::Rc;

let a = Rc::new(vec![1, 2, 3]);
let b = Rc::clone(&a); // 递增引用计数（非深拷贝）
let c = Rc::clone(&a);
println!("Ref count: {}", Rc::strong_count(&a)); // 3

// 三者指向同一个 Vec。当最后一个 Rc 被 drop 时，
// Vec 被释放。

// --- Arc<T>：多个所有者，线程安全 ---
// 使用场景：跨线程的共享所有权
use std::sync::Arc;

let shared = Arc::new(String::from("shared data"));
let handles: Vec<_> = (0..5).map(|_| {
    let shared = Arc::clone(&shared);
    std::thread::spawn(move || println!("{shared}"))
}).collect();
for h in handles { h.join().unwrap(); }
```

### Weak 引用 — 打破引用循环

`Rc` 和 `Arc` 使用引用计数，无法释放循环引用（A → B → A）。`Weak<T>` 是一个非拥有型句柄，它**不会**增加强引用计数：

```rust
use std::rc::{Rc, Weak};
use std::cell::RefCell;

struct Node {
    value: i32,
    parent: RefCell<Weak<Node>>,   // 不会保持父节点存活
    children: RefCell<Vec<Rc<Node>>>,
}

let parent = Rc::new(Node {
    value: 0, parent: RefCell::new(Weak::new()), children: RefCell::new(vec![]),
});
let child = Rc::new(Node {
    value: 1, parent: RefCell::new(Rc::downgrade(&parent)), children: RefCell::new(vec![]),
});
parent.children.borrow_mut().push(Rc::clone(&child));

// 从子节点访问父节点——返回 Option<Rc<Node>>：
if let Some(p) = child.parent.borrow().upgrade() {
    println!("Child's parent value: {}", p.value); // 0
}
// 当 `parent` 被 drop 时，strong_count → 0，内存被释放。
// 此时 `child.parent.upgrade()` 会返回 `None`。
```

**经验法则**：使用 `Rc`/`Arc` 表示所有权边，使用 `Weak` 表示反向引用和缓存。对于线程安全的代码，使用 `Arc<T>` 配合 `sync::Weak<T>`。

### Cell 和 RefCell — 内部可变性

有时你需要修改共享（`&`）引用背后的数据。Rust 通过运行时借用检查提供*内部可变性（interior mutability）*：

```rust
use std::cell::{Cell, RefCell};

// --- Cell<T>：基于 Copy 的内部可变性 ---
// 仅适用于 Copy 类型（或通过 swap 进出的类型）
struct Counter {
    count: Cell<u32>,
}

impl Counter {
    fn new() -> Self { Counter { count: Cell::new(0) } }

    fn increment(&self) { // &self，而非 &mut self！
        self.count.set(self.count.get() + 1);
    }

    fn value(&self) -> u32 { self.count.get() }
}

// --- RefCell<T>：运行时借用检查 ---
// 若运行时违反借用规则则会 panic
struct Cache {
    data: RefCell<Vec<String>>,
}

impl Cache {
    fn new() -> Self { Cache { data: RefCell::new(Vec::new()) } }

    fn add(&self, item: String) { // &self——从外部看似不可变
        self.data.borrow_mut().push(item); // 运行时检查的 &mut
    }

    fn get_all(&self) -> Vec<String> {
        self.data.borrow().clone() // 运行时检查的 &
    }

    fn bad_example(&self) {
        let _guard1 = self.data.borrow();
        // let _guard2 = self.data.borrow_mut();
        // ❌ 运行时 PANIC——存在 & 时无法获得 &mut
    }
}
```

> **Cell 对比 RefCell**：`Cell` 永远不会 panic（它复制/交换值）但只适用于 `Copy` 类型或通过 `swap()`/`replace()` 使用。`RefCell` 适用于任何类型，但在出现双重可变借用时会 panic。两者都不是 `Sync`——用于多线程时，请参见 `Mutex`/`RwLock`。

### Cow — 写时克隆

`Cow`（Clone on Write，写时克隆）持有借用的值或拥有的值。它仅在需要修改时才进行*克隆*：

```rust
use std::borrow::Cow;

// 无需修改时避免分配：
fn normalize(input: &str) -> Cow<'_, str> {
    if input.contains('\t') {
        // 仅在需要替换制表符时才分配
        Cow::Owned(input.replace('\t', "    "))
    } else {
        // 不分配——直接返回引用
        Cow::Borrowed(input)
    }
}

fn main() {
    let clean = "no tabs here";
    let dirty = "tabs\there";

    let r1 = normalize(clean); // Cow::Borrowed——零分配
    let r2 = normalize(dirty); // Cow::Owned——分配了新 String

    println!("{r1}");
    println!("{r2}");
}

// 对于可能需要所有权（也可能不需要）的函数参数也很有用：
fn process(data: Cow<'_, [u8]>) {
    // 无需拷贝即可读取数据
    println!("Length: {}", data.len());
    // 需要修改时，Cow 自动克隆：
    let mut owned = data.into_owned(); // 仅当 Borrowed 时才克隆
    owned.push(0xFF);
}
```

#### 用于二进制数据的 `Cow<'_, [u8]>`

`Cow` 对于面向字节的 API 尤其有用，这些 API 中的数据可能需要也可能不需要转换（插入校验和、填充、转义）。这避免了在常见的快速路径上分配 `Vec<u8>`：

```rust
use std::borrow::Cow;

/// 将帧填充至最小长度，无需填充时借用。
fn pad_frame(frame: &[u8], min_len: usize) -> Cow<'_, [u8]> {
    if frame.len() >= min_len {
        Cow::Borrowed(frame)  // 已足够长——零分配
    } else {
        let mut padded = frame.to_vec();
        padded.resize(min_len, 0x00);
        Cow::Owned(padded)    // 仅在需要填充时才分配
    }
}

let short = pad_frame(&[0xDE, 0xAD], 8);    // Owned——填充至 8 字节
let long  = pad_frame(&[0; 64], 8);          // Borrowed——已 ≥ 8
```

> **提示**：当你需要引用计数式地共享可能经过转换的缓冲区时，可将 `Cow<[u8]>` 与 `bytes::Bytes`（第 10 章）结合使用。

### 何时使用哪种指针

| 指针 | 所有者数量 | 线程安全 | 可变性 | 使用场景 |
|---------|:-----------:|:-----------:|:----------:|----------|
| `Box<T>` | 1 | ✅（如果 T: Send） | 通过 `&mut` | 堆分配、trait 对象、递归类型 |
| `Rc<T>` | N | ❌ | 无（用 Cell/RefCell 包装） | 共享所有权、单线程、图/树 |
| `Arc<T>` | N | ✅ | 无（用 Mutex/RwLock 包装） | 跨线程的共享所有权 |
| `Cell<T>` | — | ❌ | `.get()` / `.set()` | Copy 类型的内部可变性 |
| `RefCell<T>` | — | ❌ | `.borrow()` / `.borrow_mut()` | 任意类型的内部可变性，单线程 |
| `Cow<'_, T>` | 0 或 1 | ✅（如果 T: Send） | 写时克隆 | 数据经常不变时避免分配 |

### Pin 与自引用类型

`Pin<P>` 阻止值在内存中被移动。这对于**自引用类型（self-referential types）**——即包含指向自身数据的指针的结构体——以及对于可能在 `.await` 点之间持有引用的 `Future` 来说是至关重要的。

```rust
use std::pin::Pin;
use std::marker::PhantomPinned;

// 一个自引用结构体（简化版）：
struct SelfRef {
    data: String,
    ptr: *const String, // 指向上方的 `data`
    _pin: PhantomPinned, // 退出 Unpin——不可被移动
}

impl SelfRef {
    fn new(s: &str) -> Pin<Box<Self>> {
        let val = SelfRef {
            data: s.to_string(),
            ptr: std::ptr::null(),
            _pin: PhantomPinned,
        };
        let mut boxed = Box::pin(val);

        // SAFETY：设置指针后我们不会移动数据
        let self_ptr: *const String = &boxed.data;
        unsafe {
            let mut_ref = Pin::as_mut(&mut boxed);
            Pin::get_unchecked_mut(mut_ref).ptr = self_ptr;
        }
        boxed
    }

    fn data(&self) -> &str {
        &self.data
    }

    fn ptr_data(&self) -> &str {
        // SAFETY：指针在 pinned 时被设置为指向 self.data
        unsafe { &*self.ptr }
    }
}

fn main() {
    let pinned = SelfRef::new("hello");
    assert_eq!(pinned.data(), pinned.ptr_data()); // 两者均为 "hello"
    // std::mem::swap 会使指针失效——但 Pin 阻止了这一点
}
```

**关键概念**：

| 概念 | 含义 |
|---------|--------|
| `Unpin`（自动 trait） | "移动此类型是安全的。"大多数类型默认是 `Unpin` 的。 |
| `!Unpin` / `PhantomPinned` | "我有内部指针——不要移动我。" |
| `Pin<&mut T>` | 一个保证 `T` 不会被移动的可变引用 |
| `Pin<Box<T>>` | 一个拥有的、固定在堆上的值 |

**为什么这对 async 很重要**：每个 `async fn` 都会脱糖为一个 `Future`，它可能在 `.await` 点之间持有引用——从而使其成为自引用的。async 运行时使用 `Pin<&mut Future>` 来保证 future 在被 poll 时不会被移动。

```rust
// 当你编写：
async fn fetch(url: &str) -> String {
    let response = http_get(url).await; // 引用跨越 await 持有
    response.text().await
}

// 编译器生成一个 !Unpin 的状态机结构体，
// 运行时在调用 Future::poll() 之前将其 pin。
```

> **何时需要关心 Pin**：(1) 手动实现 `Future`，(2) 编写 async 运行时或组合子，(3) 任何带有自引用指针的结构体。对于普通的应用程序代码，`async/await` 会透明地处理 pinning。请参阅配套的《Async Rust 训练》获取更深入的讲解。
>
> **crate 替代方案**：对于不需要手动 `Pin` 的自引用结构体，可以考虑 [`ouroboros`](https://crates.io/crates/ouroboros) 或 [`self_cell`](https://crates.io/crates/self_cell)——它们能生成具有正确 pinning 和 drop 语义的安全包装器。

### Pin 投影 — 结构性 Pinning

当你拥有 `Pin<&mut MyStruct>` 时，通常需要访问各个字段。**Pin 投影（pin projection）**是从 `Pin<&mut Struct>` 安全地转换为 `Pin<&mut Field>`（对于被 pin 的字段）或 `&mut Field`（对于未被 pin 的字段）的模式。

#### 问题：被 pin 类型的字段访问

```rust
use std::pin::Pin;
use std::marker::PhantomPinned;

struct MyFuture {
    data: String,              // 普通字段——可以安全移动
    state: InternalState,      // 自引用——必须保持 pinned
    _pin: PhantomPinned,
}

enum InternalState {
    Waiting { ptr: *const String }, // 指向 `data`——自引用
    Done,
}

// 给定 `Pin<&mut MyFuture>`，如何访问 `data` 和 `state`？
// 不能直接用 `pinned.data`——编译器不允许在没有 unsafe 的情况下
// 从 pinned 值获取字段的 &mut 引用。
```

#### 手动 Pin 投影（unsafe）

```rust
impl MyFuture {
    // 投影到 `data`——此字段是结构性未 pinned 的（可安全移动）
    fn data(self: Pin<&mut Self>) -> &mut String {
        // SAFETY：`data` 不是结构性 pinned 的。单独移动 `data`
        // 不会移动整个结构体，因此 Pin 的保证得以保持。
        unsafe { &mut self.get_unchecked_mut().data }
    }

    // 投影到 `state`——此字段是结构性 pinned 的
    fn state(self: Pin<&mut Self>) -> Pin<&mut InternalState> {
        // SAFETY：`state` 是结构性 pinned 的——我们通过返回
        // Pin<&mut InternalState> 来维持 pin 不变量。
        unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().state) }
    }
}
```

**结构性 pinning 规则**——一个字段是"结构性 pinned"的，如果：
1. 单独移动/交换该字段可能使自引用失效
2. 结构体的 `Drop` 实现不得移动该字段
3. 结构体必须是 `!Unpin` 的（通过 `PhantomPinned` 或 `!Unpin` 的字段来强制）

#### `pin-project` — 安全的 Pin 投影（零 unsafe）

`pin-project` crate 在编译时生成可证明正确的投影，消除了手动 `unsafe` 的需要：

```rust
use pin_project::pin_project;
use std::pin::Pin;
use std::future::Future;
use std::task::{Context, Poll};

#[pin_project]                   // <-- 生成投影方法
struct TimedFuture<F: Future> {
    #[pin]                       // <-- 结构性 pinned（它是一个 Future）
    inner: F,
    started_at: std::time::Instant, // 非 pinned——普通数据
}

impl<F: Future> Future for TimedFuture<F> {
    type Output = (F::Output, std::time::Duration);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();  // 安全！由 pin_project 生成
        //   this.inner   : Pin<&mut F>              — pinned 字段
        //   this.started_at : &mut std::time::Instant — 未 pinned 字段

        match this.inner.poll(cx) {
            Poll::Ready(output) => {
                let elapsed = this.started_at.elapsed();
                Poll::Ready((output, elapsed))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
```

#### `pin-project` 对比手动投影

| 方面 | 手动（`unsafe`） | `pin-project` |
|--------|-------------------|---------------|
| 安全性 | 你来证明不变量 | 编译器验证 |
| 样板代码 | 少（但容易出错） | 零——derive 宏 |
| `Drop` 交互 | 不得移动 pinned 字段 | 强制执行：`#[pinned_drop]` |
| 编译时开销 | 无 | 过程宏展开 |
| 使用场景 | 基础类型、`no_std` | 应用程序 / 库代码 |

#### `#[pinned_drop]` — 被 pin 类型的 Drop

当一个类型有 `#[pin]` 字段时，`pin-project` 要求使用 `#[pinned_drop]` 而非常规的 `Drop` 实现，以防止意外移动被 pin 的字段：

```rust
use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;

#[pin_project(PinnedDrop)]
struct Connection<F> {
    #[pin]
    future: F,
    buffer: Vec<u8>,  // 非 pinned——可在 drop 时移动
}

#[pinned_drop]
impl<F> PinnedDrop for Connection<F> {
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();
        // `this.future` 是 Pin<&mut F>——不可移动，只能就地 drop
        // `this.buffer` 是 &mut Vec<u8>——可以清空、释放等操作
        this.buffer.clear();
        println!("Connection dropped, buffer cleared");
    }
}
```

#### Pin 投影在实践中何时重要

> **注意**：下图使用 Mermaid 语法。它可以在 GitHub 和支持 Mermaid 的工具中渲染（带 `mermaid` 插件的 mdBook、带 Mermaid 扩展的 VS Code）。在纯 Markdown 查看器中，你会看到原始源代码。

```mermaid
graph TD
    A["你是否手动实现 Future？"] -->|是| B["该 future 是否在<br/>.await 点之间持有引用？"]
    A -->|否| C["async/await 会为你处理 Pin<br/>✅ 无需投影"]
    B -->|是| D["在你的 future 结构体上<br/>使用 #[pin_project]"]
    B -->|否| E["你的 future 是 Unpin<br/>✅ 无需投影"]
    D --> F["将 future/stream 标记为 #[pin]<br/>数据字段保持未 pin"]
    
    style C fill:#91e5a3,color:#000
    style E fill:#91e5a3,color:#000
    style D fill:#ffa07a,color:#000
    style F fill:#ffa07a,color:#000
```

> **经验法则**：如果你在包装另一个 `Future` 或 `Stream`，请使用 `pin-project`。如果你在用 `async/await` 编写应用程序代码，你永远不需要直接使用 pin 投影。请参阅配套的《Async Rust 训练》了解使用 pin 投影的 async 组合子模式。

### Drop 顺序与 ManuallyDrop

Rust 的 drop 顺序是确定性的，但有一些值得了解的规则：

#### Drop 顺序规则

```rust
struct Label(&'static str);

impl Drop for Label {
    fn drop(&mut self) { println!("Dropping {}", self.0); }
}

fn main() {
    let a = Label("first");   // 先声明
    let b = Label("second");  // 后声明
    let c = Label("third");   // 最后声明
}
// 输出：
//   Dropping third    ← 局部变量按逆声明顺序 drop
//   Dropping second
//   Dropping first
```

**三条规则**：

| 内容 | Drop 顺序 | 理由 |
|------|-----------|----------|
| **局部变量** | 逆声明顺序 | 后声明的变量可能引用先声明的 |
| **结构体字段** | 声明顺序（从上到下） | 与构造顺序一致（自 Rust 1.0 起稳定，由 [RFC 1857](https://rust-lang.github.io/rfcs/1857-stabilize-drop-order.html) 保证） |
| **元组元素** | 声明顺序（从左到右） | `(a, b, c)` → 先 drop `a`，然后 `b`，然后 `c` |

```rust
struct Server {
    listener: Label,  // 第 1 个 drop
    handler: Label,   // 第 2 个 drop
    logger: Label,    // 第 3 个 drop
}
// 字段按从上到下（声明顺序）drop。
// 当字段相互引用或持有资源时，这一点很重要。
```

> **实际影响**：如果你的结构体有一个 `JoinHandle` 和一个 `Sender`，字段顺序决定了哪个先 drop。如果线程从通道读取，先 drop `Sender`（关闭通道）让线程退出，然后 join handle。在结构体中将 `Sender` 放在 `JoinHandle` 上方。

#### `ManuallyDrop<T>` — 抑制自动 Drop

`ManuallyDrop<T>` 包装一个值并阻止其析构函数自动运行。你负责 drop 它（或故意泄漏它）：

```rust
use std::mem::ManuallyDrop;

// 用例 1：在 unsafe 代码中防止双重释放
struct TwoPhaseBuffer {
    // 我们需要自行 drop Vec 以控制时机
    data: ManuallyDrop<Vec<u8>>,
    committed: bool,
}

impl TwoPhaseBuffer {
    fn new(capacity: usize) -> Self {
        TwoPhaseBuffer {
            data: ManuallyDrop::new(Vec::with_capacity(capacity)),
            committed: false,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn commit(&mut self) {
        self.committed = true;
        println!("Committed {} bytes", self.data.len());
    }
}

impl Drop for TwoPhaseBuffer {
    fn drop(&mut self) {
        if !self.committed {
            println!("Rolling back — dropping uncommitted data");
        }
        // SAFETY：data 在此处始终有效；我们只 drop 一次。
        unsafe { ManuallyDrop::drop(&mut self.data); }
    }
}
```

```rust
// 用例 2：故意泄漏（例如全局单例）
fn leaked_string() -> &'static str {
    // Box::leak() 是创建 &'static 引用的惯用方式：
    let s = String::from("lives forever");
    Box::leak(s.into_boxed_str())
    // ⚠️ 这是一次受控的内存泄漏。String 的堆分配
    // 永远不会被释放。仅用于长期存活的单例。
}

// ManuallyDrop 替代方案（需要 unsafe）：
// ⚠️ 优先使用上方的 Box::leak()——此处仅为说明
// ManuallyDrop 语义（在堆数据存活时抑制 Drop）。
fn leaked_string_manual() -> &'static str {
    use std::mem::ManuallyDrop;
    let md = ManuallyDrop::new(String::from("lives forever"));
    // SAFETY：ManuallyDrop 阻止释放；堆数据永久存活，
    // 因此 'static 引用是有效的。
    unsafe { &*(md.as_str() as *const str) }
}
```

```rust
// 用例 3：联合体字段（同一时间只有一个变体有效）
use std::mem::ManuallyDrop;

union IntOrString {
    i: u64,
    s: ManuallyDrop<String>,
    // String 有 Drop 实现，因此在联合体中必须用 ManuallyDrop 包装
    // ——编译器无法知道哪个字段是活跃的。
}

// 无自动 Drop——构造 IntOrString 的代码也必须
// 负责清理。如果 String 变体是活跃的，调用：
//   unsafe { ManuallyDrop::drop(&mut value.s); }
// 没有 Drop 实现时，联合体直接被泄漏（无 UB，仅是泄漏）。
```

**ManuallyDrop 对比 `mem::forget`**：

| | `ManuallyDrop<T>` | `mem::forget(value)` |
|---|---|---|
| 何时 | 在构造时包装 | 稍后消耗 |
| 访问内部 | `&*md` / `&mut *md` | 值已消失 |
| 稍后 drop | `ManuallyDrop::drop(&mut md)` | 不可能 |
| 使用场景 | 细粒度生命周期控制 | 即发即忘的泄漏 |

> **规则**：在需要*精确*控制析构函数何时运行的不安全抽象中使用 `ManuallyDrop`。在安全的应用程序代码中，你几乎永远不需要它——Rust 的自动 drop 顺序会正确处理一切。

> **关键要点 — 智能指针**
> - `Box` 用于堆上的单一所有权；`Rc`/`Arc` 用于共享所有权（单线程/多线程）
> - `Cell`/`RefCell` 提供内部可变性；`RefCell` 在运行时违反借用规则时 panic
> - `Cow` 避免在常见路径上分配内存；`Pin` 阻止自引用类型被移动
> - Drop 顺序：字段按声明顺序 drop（RFC 1857）；局部变量按逆声明顺序 drop

> **另请参阅：**[第 6 章 — 并发](ch06-concurrency-vs-parallelism-vs-threads.md) 了解 Arc + Mutex 模式。[第 4 章 — PhantomData](ch04-phantomdata-types-that-carry-no-data.md) 了解与智能指针配合使用的 PhantomData。

```mermaid
graph TD
    Box["Box&lt;T&gt;<br>单一所有者，堆"] --> Heap["堆分配"]
    Rc["Rc&lt;T&gt;<br>共享，单线程"] --> Heap
    Arc["Arc&lt;T&gt;<br>共享，多线程"] --> Heap

    Rc --> Weak1["Weak&lt;T&gt;<br>非拥有型"]
    Arc --> Weak2["Weak&lt;T&gt;<br>非拥有型"]

    Cell["Cell&lt;T&gt;<br>Copy 内部可变性"] --> Stack["栈 / 内部"]
    RefCell["RefCell&lt;T&gt;<br>运行时借用检查"] --> Stack
    Cow["Cow&lt;T&gt;<br>写时克隆"] --> Stack

    style Box fill:#d4efdf,stroke:#27ae60,color:#000
    style Rc fill:#e8f4f8,stroke:#2980b9,color:#000
    style Arc fill:#e8f4f8,stroke:#2980b9,color:#000
    style Weak1 fill:#fef9e7,stroke:#f1c40f,color:#000
    style Weak2 fill:#fef9e7,stroke:#f1c40f,color:#000
    style Cell fill:#fdebd0,stroke:#e67e22,color:#000
    style RefCell fill:#fdebd0,stroke:#e67e22,color:#000
    style Cow fill:#fdebd0,stroke:#e67e22,color:#000
    style Heap fill:#f5f5f5,stroke:#999,color:#000
    style Stack fill:#f5f5f5,stroke:#999,color:#000
```

---

### 练习：引用计数图 ★★（约 30 分钟）

使用 `Rc<RefCell<Node>>` 构建一个有向图，其中每个节点有一个名称和一个子节点列表。使用 `Weak` 打破反向边来创建一个循环（A → B → C → A）。用 `Rc::strong_count` 验证没有内存泄漏。

<details>
<summary>🔑 解答</summary>

```rust
use std::cell::RefCell;
use std::rc::{Rc, Weak};

struct Node {
    name: String,
    children: Vec<Rc<RefCell<Node>>>,
    back_ref: Option<Weak<RefCell<Node>>>,
}

impl Node {
    fn new(name: &str) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Node {
            name: name.to_string(),
            children: Vec::new(),
            back_ref: None,
        }))
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        println!("Dropping {}", self.name);
    }
}

fn main() {
    let a = Node::new("A");
    let b = Node::new("B");
    let c = Node::new("C");

    // A → B → C，其中 C 通过 Weak 反向引用 A
    a.borrow_mut().children.push(Rc::clone(&b));
    b.borrow_mut().children.push(Rc::clone(&c));
    c.borrow_mut().back_ref = Some(Rc::downgrade(&a)); // Weak 引用！

    println!("A strong count: {}", Rc::strong_count(&a)); // 1（仅 `a` 绑定）
    println!("B strong count: {}", Rc::strong_count(&b)); // 2（b + A 的子节点）
    println!("C strong count: {}", Rc::strong_count(&c)); // 2（c + B 的子节点）

    // 升级 weak 引用以验证其工作：
    let c_ref = c.borrow();
    if let Some(back) = &c_ref.back_ref {
        if let Some(a_ref) = back.upgrade() {
            println!("C points back to: {}", a_ref.borrow().name);
        }
    }
    // 当 a、b、c 离开作用域时，所有 Node 都被 drop（无循环泄漏！）
}
```

</details>

***
