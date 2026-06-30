# 12. Unsafe Rust——可控的危险 🔴

> **你将学到：**
> - 五种 unsafe 超能力及各自的使用时机
> - 编写健全的抽象：安全的 API，unsafe 的内部实现
> - 从 Rust 调用 C 的 FFI 模式（以及反向调用）
> - 常见的未定义行为（UB）陷阱与 arena/slab 分配器模式

## 五种 Unsafe 超能力

`unsafe` 解锁了编译器无法验证的五种操作：

```rust
// SAFETY: 下面逐一解释每项操作。
unsafe {
    // 1. 解引用裸指针
    let ptr: *const i32 = &42;
    let value = *ptr; // 可能是悬垂/空指针

    // 2. 调用 unsafe 函数
    let layout = std::alloc::Layout::new::<u64>();
    let mem = std::alloc::alloc(layout);

    // 3. 访问可变静态变量
    static mut COUNTER: u32 = 0;
    COUNTER += 1; // 多线程访问时会产生数据竞争

    // 4. 实现 unsafe trait
    // unsafe impl Send for MyType {}

    // 5. 访问 union 的字段
    // union IntOrFloat { i: i32, f: f32 }
    // let u = IntOrFloat { i: 42 };
    // let f = u.f; // 重新解释位模式——可能是垃圾值
}
```

> **关键原则**：`unsafe` 不会关闭借用检查器或类型系统。
> 它只解锁这五种特定能力。所有其他 Rust 规则仍然适用。

### 编写健全的抽象

`unsafe` 的目的是围绕 unsafe 操作构建**安全的抽象**：

```rust
/// 一个固定容量的栈分配缓冲区。
/// 所有公共方法都是安全的——unsafe 被封装在内部。
pub struct StackBuf<T, const N: usize> {
    data: [std::mem::MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> StackBuf<T, N> {
    pub fn new() -> Self {
        StackBuf {
            // 每个元素各自是 MaybeUninit——不需要 unsafe。
            // `const { ... }` 块（Rust 1.79+）允许我们将非 Copy
            // 的常量表达式重复 N 次。
            data: [const { std::mem::MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.len >= N {
            return Err(value); // 缓冲区已满——将值返回给调用者
        }
        // SAFETY: len < N，因此 data[len] 在边界内。
        // 我们向 MaybeUninit 槽位写入一个有效的 T。
        self.data[self.len] = std::mem::MaybeUninit::new(value);
        self.len += 1;
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            // SAFETY: index < len，且 data[0..len] 全部已初始化。
            Some(unsafe { self.data[index].assume_init_ref() })
        } else {
            None
        }
    }
}

impl<T, const N: usize> Drop for StackBuf<T, N> {
    fn drop(&mut self) {
        // SAFETY: data[0..len] 已初始化——正确地逐个 drop。
        for i in 0..self.len {
            unsafe { self.data[i].assume_init_drop(); }
        }
    }
}
```

**健全 unsafe 代码的三条规则**：
1. **文档化不变量**——每条 `// SAFETY:` 注释解释为什么操作是有效的
2. **封装**——unsafe 位于安全的 API 内部；用户无法触发 UB
3. **最小化**——只有尽可能小的代码块是 `unsafe` 的

### FFI 模式：从 Rust 调用 C

```rust
// 声明 C 函数签名：
extern "C" {
    fn strlen(s: *const std::ffi::c_char) -> usize;
    fn printf(format: *const std::ffi::c_char, ...) -> std::ffi::c_int;
}

// 安全封装：
fn safe_strlen(s: &str) -> usize {
    let c_string = std::ffi::CString::new(s).expect("string contains null byte");
    // SAFETY: c_string 是有效的以 null 结尾的字符串，在调用期间保持存活。
    unsafe { strlen(c_string.as_ptr()) }
}

// 从 C 调用 Rust（导出函数）：
#[no_mangle]
pub extern "C" fn rust_add(a: i32, b: i32) -> i32 {
    a + b
}
```

**常见的 FFI 类型**：

| Rust | C | 说明 |
|------|---|-------|
| `i32` / `u32` | `int32_t` / `uint32_t` | 固定宽度，安全 |
| `*const T` / `*mut T` | `const T*` / `T*` | 裸指针 |
| `std::ffi::CStr` | `const char*`（借用） | 以 null 结尾，借用 |
| `std::ffi::CString` | `char*`（拥有） | 以 null 结尾，拥有 |
| `std::ffi::c_void` | `void` | 不透明指针目标 |
| `Option<fn(...)>` | 可空函数指针 | `None` = NULL |

### 常见的未定义行为（UB）陷阱

| 陷阱 | 示例 | 为何是 UB |
|---------|---------|------------|
| 空指针解引用 | `*std::ptr::null::<i32>()` | 解引用空指针始终是 UB |
| 悬垂指针 | `drop()` 后解引用 | 内存可能已被复用 |
| 数据竞争 | 两个线程写入 `static mut` | 未同步的并发写入 |
| 错误的 `assume_init` | `MaybeUninit::<String>::uninit().assume_init()` | 读取未初始化内存。**注意**：`[const { MaybeUninit::uninit() }; N]`（Rust 1.79+）是创建 `MaybeUninit` 数组的安全方式——不需要 `unsafe` 或 `assume_init`（参见上文的 `StackBuf::new()`）。 |
| 别名违规 | 为同一数据创建两个 `&mut` | 违反 Rust 的别名模型 |
| 无效的枚举值 | `std::mem::transmute::<u8, bool>(2)` | `bool` 只能是 0 或 1 |

> **生产环境何时使用 `unsafe`**：
> - FFI 边界（调用 C/C++ 代码）
> - 性能关键的内部循环（避免边界检查）
> - 构建原语（`Vec`、`HashMap`——它们内部使用 unsafe）
> - 应用逻辑中能避免就避免

### 自定义分配器——Arena 与 Slab 模式

在 C 中，你会为特定的分配模式编写自定义 `malloc()` 替代品——一次性释放所有内容的 arena 分配器、用于固定大小对象的 slab 分配器，或用于高吞吐系统的池分配器。Rust 通过 `GlobalAlloc` trait 和分配器 crate 提供了相同的能力，并增加了生命周期作用域的 arena 这一优势，能够**在编译期防止 use-after-free**。

#### Arena 分配器——批量分配，批量释放

Arena 通过向前推进指针来分配。单个对象无法被释放——整个 arena 一次性释放。这非常适合请求作用域或帧作用域的分配：

```rust
use bumpalo::Bump;

fn process_sensor_frame(raw_data: &[u8]) {
    // 为这一帧的分配创建一个 arena
    let arena = Bump::new();

    // 在 arena 中分配对象——每个约 2ns（仅推进指针）
    let header = arena.alloc(parse_header(raw_data));
    let readings: &mut [f32] = arena.alloc_slice_fill_default(header.sensor_count);

    for (i, chunk) in raw_data[header.payload_offset..].chunks(4).enumerate() {
        if i < readings.len() {
            readings[i] = f32::from_le_bytes(chunk.try_into().unwrap());
        }
    }

    // 使用 readings...
    let avg = readings.iter().sum::<f32>() / readings.len() as f32;
    println!("Frame avg: {avg:.2}");

    // `arena` 在此处 drop——所有分配在 O(1) 内一次性释放
    // 无逐对象析构开销，无内存碎片
}
# fn parse_header(_: &[u8]) -> Header { Header { sensor_count: 4, payload_offset: 8 } }
# struct Header { sensor_count: usize, payload_offset: usize }
```

**Arena 与标准分配器对比**：

| 方面 | `Vec::new()` / `Box::new()` | `Bump` arena |
|--------|---------------------------|--------------|
| 分配速度 | ~25ns（malloc） | ~2ns（指针推进） |
| 释放速度 | 逐对象析构 | O(1) 批量释放 |
| 内存碎片 | 有（长时间运行的进程） | arena 内无碎片 |
| 生命周期安全 | 堆——在 `Drop` 时释放 | Arena 引用——编译期作用域 |
| 使用场景 | 通用 | 请求/帧/批处理 |

#### `typed-arena`——类型安全的 Arena

当所有 arena 对象类型相同时，`typed-arena` 提供了更简洁的 API，返回带有 arena 生命周期的引用：

```rust
use typed_arena::Arena;

struct AstNode<'a> {
    value: i32,
    children: Vec<&'a AstNode<'a>>,
}

fn build_tree() {
    let arena: Arena<AstNode<'_>> = Arena::new();

    // 分配节点——返回与 arena 生命周期绑定的 &AstNode
    let root = arena.alloc(AstNode { value: 1, children: vec![] });
    let left = arena.alloc(AstNode { value: 2, children: vec![] });
    let right = arena.alloc(AstNode { value: 3, children: vec![] });

    // 构建树——只要 `arena` 存活，所有引用都有效
    // （对于真正可变的树，可变性访问需要内部可变性）

    println!("Root: {}, Left: {}, Right: {}", root.value, left.value, right.value);

    // `arena` 在此处 drop——所有节点一次性释放
}
```

#### Slab 分配器——固定大小对象池

Slab 分配器预分配一个固定大小槽位的池。对象被单独分配和归还，但所有槽位大小相同——消除了内存碎片并实现了 O(1) 的分配/释放：

```rust
use slab::Slab;

struct Connection {
    id: u64,
    buffer: [u8; 1024],
    active: bool,
}

fn connection_pool_example() {
    // 为连接预分配一个 slab
    let mut connections: Slab<Connection> = Slab::with_capacity(256);

    // insert 返回一个 key（usize 索引）—— O(1)
    let key1 = connections.insert(Connection {
        id: 1001,
        buffer: [0; 1024],
        active: true,
    });

    let key2 = connections.insert(Connection {
        id: 1002,
        buffer: [0; 1024],
        active: true,
    });

    // 通过 key 访问—— O(1)
    if let Some(conn) = connections.get_mut(key1) {
        conn.buffer[0..5].copy_from_slice(b"hello");
    }

    // remove 返回值—— O(1)，槽位被下次 insert 复用
    let removed = connections.remove(key2);
    assert_eq!(removed.id, 1002);

    // 下次 insert 复用已释放的槽位——无内存碎片
    let key3 = connections.insert(Connection {
        id: 1003,
        buffer: [0; 1024],
        active: true,
    });
    assert_eq!(key3, key2); // 复用了同一个槽位！
}
```

#### 实现最小化 Arena（用于 `no_std`）

对于无法引入 `bumpalo` 的裸机环境，以下是一个基于 `unsafe` 构建的最小化 arena：

```rust
#![cfg_attr(not(test), no_std)]

use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};

/// 一个由固定大小字节数组支持的简单 bump 分配器。
/// 非线程安全——在多线程环境中应使用每核独立实例或加锁。
///
/// **重要**：与 `bumpalo` 一样，该 arena 在 drop 时不会调用
/// 已分配项的析构函数。实现了 `Drop` 的类型会泄漏其资源
/// （文件句柄、套接字等）。仅分配没有有意义 `Drop` 实现的类型，
/// 或者在 arena 释放前手动 drop 它们。
pub struct FixedArena<const N: usize> {
    // 此处必须使用 UnsafeCell：我们通过 &self 来修改 buf。
    // 没有 UnsafeCell，将 &self.buf 转换为 *mut u8 将是 UB
    // （违反 Rust 的别名模型——共享引用意味着不可变）。
    buf: UnsafeCell<[u8; N]>,
    offset: Cell<usize>, // 为 &self 分配提供内部可变性
}

impl<const N: usize> FixedArena<N> {
    pub const fn new() -> Self {
        FixedArena {
            buf: UnsafeCell::new([0; N]),
            offset: Cell::new(0),
        }
    }

    /// 在 arena 中分配一个 `T`。空间不足时返回 `None`。
    pub fn alloc<T>(&self, value: T) -> Option<&mut T> {
        let layout = Layout::new::<T>();
        let current = self.offset.get();

        // 对齐向上取整
        let aligned = (current + layout.align() - 1) & !(layout.align() - 1);
        let new_offset = aligned + layout.size();

        if new_offset > N {
            return None; // Arena 已满
        }

        self.offset.set(new_offset);

        // SAFETY:
        // - `aligned` 在 `buf` 边界内（上面已检查）
        // - 对齐正确（按 T 的要求对齐）
        // - 无别名：每次 alloc 返回唯一的、不重叠的区域
        // - UnsafeCell 授权通过 &self 进行修改
        // - arena 的生命周期长于返回的引用（调用者须保证）
        let ptr = unsafe {
            let base = (self.buf.get() as *mut u8).add(aligned);
            let typed = base as *mut T;
            typed.write(value);
            &mut *typed
        };

        Some(ptr)
    }

    /// 重置 arena——使所有先前的分配失效。
    ///
    /// # Safety
    /// 调用者必须确保不存在对 arena 已分配数据的引用。
    pub unsafe fn reset(&self) {
        self.offset.set(0);
    }

    pub fn used(&self) -> usize {
        self.offset.get()
    }

    pub fn remaining(&self) -> usize {
        N - self.offset.get()
    }
}
```

#### 选择分配器策略

> **注意**：下图使用 Mermaid 语法。它在 GitHub 和支持 Mermaid 的工具中渲染
> （带 `mermaid` 插件的 mdBook、带 Mermaid 扩展的 VS Code）。在纯 Markdown 查看器中，
> 你会看到原始源码。

```mermaid
graph TD
    A["你的分配模式是什么？"] --> B{都是相同类型？}
    A --> I{"环境？"}
    B -->|是| C{需要单独释放？}
    B -->|否| D{需要单独释放？}
    C -->|是| E["<b>Slab</b><br/>slab crate<br/>O(1) 分配 + 释放<br/>基于索引的访问"]
    C -->|否| F["<b>typed-arena</b><br/>批量分配，批量释放<br/>生命周期作用域引用"]
    D -->|是| G["<b>标准分配器</b><br/>Box、Vec 等<br/>通用 malloc"]
    D -->|否| H["<b>Bump arena</b><br/>bumpalo crate<br/>~2ns 分配，O(1) 批量释放"]
    
    I -->|no_std| J["FixedArena（自定义）<br/>或 embedded-alloc"]
    I -->|std| K["bumpalo / typed-arena / slab"]
    
    style E fill:#91e5a3,color:#000
    style F fill:#91e5a3,color:#000
    style G fill:#89CFF0,color:#000
    style H fill:#91e5a3,color:#000
    style J fill:#ffa07a,color:#000
    style K fill:#91e5a3,color:#000
```

| C 模式 | Rust 等价物 | 关键优势 |
|-----------|----------------|---------------|
| 自定义 `malloc()` 池 | `#[global_allocator]` 实现 | 类型安全、可调试 |
| `obstack`（GNU） | `bumpalo::Bump` | 生命周期作用域、无 use-after-free |
| 内核 slab（`kmem_cache`） | `slab::Slab<T>` | 类型安全、基于索引 |
| 栈分配临时缓冲区 | `FixedArena<N>`（上文） | 无堆、可 `const` 构造 |
| `alloca()` | `[T; N]` 或 `SmallVec` | 编译期确定大小、无 UB |

> **交叉引用**：关于裸机分配器设置（`embedded-alloc` 的 `#[global_allocator]`），
> 请参阅《面向 C 程序员的 Rust 培训》第 15.1 章"全局分配器设置"，该章涵盖了
> 嵌入式特定的引导启动。

> **关键要点——Unsafe Rust**
> - 文档化不变量（`SAFETY:` 注释），封装在安全 API 之后，最小化 unsafe 作用域
> - `[const { MaybeUninit::uninit() }; N]`（Rust 1.79+）取代了旧的 `assume_init` 反模式
> - FFI 需要 `extern "C"`、`#[repr(C)]` 以及仔细的空指针/生命周期处理
> - Arena 和 slab 分配器以通用灵活性换取分配速度

> **另请参阅：**[第 4 章——PhantomData](ch04-phantomdata-types-that-carry-no-data.md)了解变体（variance）和 drop 检查与 unsafe 代码的交互。[第 8 章——智能指针](ch09-smart-pointers-and-interior-mutability.md)了解 Pin 和自引用类型。

---

### 练习：围绕 Unsafe 的安全封装 ★★★（约 45 分钟）

编写一个 `FixedVec<T, const N: usize>`——固定容量、栈分配的向量。
要求：
- `push(&mut self, value: T) -> Result<(), T>` 在满时返回 `Err(value)`
- `pop(&mut self) -> Option<T>` 返回并移除最后一个元素
- `as_slice(&self) -> &[T]` 借用已初始化的元素
- 所有公共方法必须是安全的；所有 unsafe 必须封装并带有 `SAFETY:` 注释
- `Drop` 必须清理已初始化的元素

<details>
<summary>🔑 解答</summary>

```rust
use std::mem::MaybeUninit;

pub struct FixedVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> FixedVec<T, N> {
    pub fn new() -> Self {
        FixedVec {
            data: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.len >= N { return Err(value); }
        // SAFETY: len < N，因此 data[len] 在边界内。
        self.data[self.len] = MaybeUninit::new(value);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 { return None; }
        self.len -= 1;
        // SAFETY: data[len] 已初始化（递减前 len > 0）。
        Some(unsafe { self.data[self.len].assume_init_read() })
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: data[0..len] 全部已初始化，且 MaybeUninit<T>
        // 与 T 具有相同的内存布局。
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const T, self.len) }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

impl<T, const N: usize> Drop for FixedVec<T, N> {
    fn drop(&mut self) {
        // SAFETY: data[0..len] 已初始化——逐个 drop。
        for i in 0..self.len {
            unsafe { self.data[i].assume_init_drop(); }
        }
    }
}

fn main() {
    let mut v = FixedVec::<String, 4>::new();
    v.push("hello".into()).unwrap();
    v.push("world".into()).unwrap();
    assert_eq!(v.as_slice(), &["hello", "world"]);
    assert_eq!(v.pop(), Some("world".into()));
    assert_eq!(v.len(), 1);
}
```

</details>

***
