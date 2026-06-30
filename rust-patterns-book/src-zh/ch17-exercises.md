## 练习

### 练习 1：类型安全状态机 ★★（约 30 分钟）

使用类型状态（type-state）模式构建一个交通灯状态机。灯必须按 `Red → Green → Yellow → Red` 转换，其他任何顺序都不可能。

<details>
<summary>🔑 解答</summary>

```rust
// → std::marker::PhantomData<T>：零大小标记类型，用于在泛型结构体中
//   "声明"对类型参数 T 的逻辑关系，而不实际存储 T 的值。
use std::marker::PhantomData;

// → Red/Green/Yellow 是零大小单元结构体，仅作为类型状态的"标签"（typestate）。
struct Red;
struct Green;
struct Yellow;

// → TrafficLight<State>：按状态参数化。运行时无状态数据，
//   状态信息完全存在于类型层面（编译期）。
struct TrafficLight<State> {
    _state: PhantomData<State>,
}

// → impl TrafficLight<Red>：只为 Red 状态实现 new 和 go。
//   编译器据此限制：只有红灯能调用 go。
impl TrafficLight<Red> {
    fn new() -> Self {
        println!("🔴 Red — STOP");
        TrafficLight { _state: PhantomData }
    }

    // → go(self) -> TrafficLight<Green>：消耗 self（所有权转移），
    //   返回新状态。旧的 Red 实例已失效，防止重复转换。
    fn go(self) -> TrafficLight<Green> {
        println!("🟢 Green — GO");
        TrafficLight { _state: PhantomData }
    }
}

impl TrafficLight<Green> {
    fn caution(self) -> TrafficLight<Yellow> {
        println!("🟡 Yellow — CAUTION");
        TrafficLight { _state: PhantomData }
    }
}

impl TrafficLight<Yellow> {
    fn stop(self) -> TrafficLight<Red> {
        println!("🔴 Red — STOP");
        TrafficLight { _state: PhantomData }
    }
}

fn main() {
    let light = TrafficLight::new(); // Red
    let light = light.go();          // Green
    let light = light.caution();     // Yellow
    let light = light.stop();        // Red

    // light.caution(); // ❌ 编译错误：Red 上没有方法 `caution`
    // TrafficLight::new().stop(); // ❌ 编译错误：Red 上没有方法 `stop`
}
```

**关键要点**：非法转换是编译错误，而非运行时 panic。

</details>

---

### 练习 2：带 PhantomData 的计量单位 ★★（约 30 分钟）

扩展第 4 章的计量单位（unit-of-measure）模式以支持：
- `Meters`、`Seconds`、`Kilograms`
- 相同单位的加法
- 乘法：`Meters * Meters = SquareMeters`
- 除法：`Meters / Seconds = MetersPerSecond`

<details>
<summary>🔑 解答</summary>

```rust
use std::marker::PhantomData;
// → std::ops 中的运算符 trait：实现它们即可重载 +、*、/ 等运算符。
//   Add = +、Mul = *、Div = /，每个 trait 有关联类型 Output。
use std::ops::{Add, Mul, Div};

// → 单元标签（零大小），代表物理量纲。
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
// → Qty<U>：带量纲标签的数值。PhantomData<U> 在编译期携带单位信息，
//   运行时零开销。类型系统据此拒绝单位不匹配的运算。
struct Qty<U> {
    value: f64,
    _unit: PhantomData<U>,
}

// → impl<U> Qty<U>：为所有单位 U 通用实现 new。
impl<U> Qty<U> {
    fn new(v: f64) -> Self { Qty { value: v, _unit: PhantomData } }
}

// → impl<U> Add for Qty<U>：实现 + 运算符。
//   type Output = Qty<U>：相同单位相加得同单位（类型层面保证）。
//   两个不同单位的 Qty 无法相加（类型 U 不同）。
impl<U> Add for Qty<U> {
    type Output = Qty<U>;
    fn add(self, rhs: Self) -> Self::Output { Qty::new(self.value + rhs.value) }
}

// → impl Mul<Qty<Meters>> for Qty<Meters>：为特定单位组合实现 *。
//   Output = Qty<SquareMeters>：米 × 米 = 平方米，单位转换在类型层完成。
impl Mul<Qty<Meters>> for Qty<Meters> {
    type Output = Qty<SquareMeters>;
    fn mul(self, rhs: Qty<Meters>) -> Qty<SquareMeters> {
        Qty::new(self.value * rhs.value)
    }
}

impl Div<Qty<Seconds>> for Qty<Meters> {
    // → 米 / 秒 = 米每秒，单位推导由类型系统在编译期完成。
    type Output = Qty<MetersPerSecond>;
    fn div(self, rhs: Qty<Seconds>) -> Qty<MetersPerSecond> {
        Qty::new(self.value / rhs.value)
    }
}

fn main() {
    // → Qty::<Meters>::new(...)：turbofish 显式指定单位类型 U。
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

    // let bad = width + time; // ❌ 编译错误：不能 Meters + Seconds 相加
}
```

</details>

---

### 练习 3：基于通道的工作池 ★★★（约 45 分钟）

使用通道构建一个工作池，其中：
- 分发器（dispatcher）通过通道发送 `Job` 结构体
- N 个工作线程消费任务并发回结果
- 使用 `crossbeam-channel`（若 crossbeam 不可用则用 `std::sync::mpsc`）

<details>
<summary>🔑 解答</summary>

```rust
// → std::sync::mpsc：多生产者单消费者通道。Sender 可 clone（多发送端），
//   Receiver 唯一（单接收端）。send/recv 跨线程通信。
use std::sync::mpsc;
use std::thread;

struct Job {
    id: u64,
    data: String,
}

struct JobResult {
    job_id: u64,
    output: String,
    worker_id: usize,
}

fn worker_pool(jobs: Vec<Job>, num_workers: usize) -> Vec<JobResult> {
    // → mpsc::channel()：返回 (Sender, Receiver) 元组。
    //   job 通道：分发器→工作线程；result 通道：工作线程→收集器。
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (result_tx, result_rx) = mpsc::channel::<JobResult>();

    // 将接收端包裹在 Arc<Mutex> 中以便工作线程共享
    // → Arc<Mutex<Receiver>>：mpsc 的 Receiver 不可 clone，需共享时用
    //   Arc（多线程共享所有权）+ Mutex（互斥访问）模拟多消费者。
    let job_rx = std::sync::Arc::new(std::sync::Mutex::new(job_rx));

    // 派生工作线程
    let mut handles = Vec::new();
    for worker_id in 0..num_workers {
        // → clone Arc 增加引用计数（共享同一 Receiver）；
        //   clone Sender 创建新的发送端副本。
        let job_rx = job_rx.clone();
        let result_tx = result_tx.clone();
        // → thread::spawn(move || ...)：创建 OS 线程，move 闭包获取所有权。
        handles.push(thread::spawn(move || {
            loop {
                // 加锁、接收、解锁 — 临界区很短
                // → 关键技巧：用块作用域限定锁的生命周期。
                //   lock() 持有期间 recv() 阻塞会卡住所有工作线程，
                //   故只在拿值的瞬间持锁，recv 在持锁内完成。
                let job = {
                    let rx = job_rx.lock().unwrap();
                    rx.recv() // 阻塞直到有任务或通道关闭
                };
                match job {
                    Ok(job) => {
                        let output = format!("processed '{}' by worker {worker_id}", job.data);
                        // → Sender::send：发送值到通道，返回 Result（接收端全 drop 时 Err）。
                        result_tx.send(JobResult {
                            job_id: job.id,
                            output,
                            worker_id,
                        }).unwrap();
                    }
                    Err(_) => break, // 通道关闭 — 退出
                }
            }
        }));
    }
    // → drop(result_tx)：丢弃收集端的发送副本。所有工作线程的发送端
    //   drop 后，result_rx 的迭代才会结束（通道关闭）。
    drop(result_tx); // 丢弃我们的副本，这样工作线程结束后结果通道会关闭

    // 分发任务
    let num_jobs = jobs.len();
    for job in jobs {
        job_tx.send(job).unwrap();
    }
    // → drop(job_tx)：关闭任务通道。工作线程 recv() 收到 Err 后退出循环。
    drop(job_tx); // 关闭任务通道 — 工作线程排空后退出

    // 收集结果
    let mut results = Vec::new();
    // → for result in result_rx：Receiver 实现 Iterator，
    //   通道关闭（所有 Sender drop）且缓冲耗尽时迭代结束。
    for result in result_rx {
        results.push(result);
    }
    assert_eq!(results.len(), num_jobs);

    // → JoinHandle::join()：阻塞等待线程结束，返回 Result（panic 时 Err）。
    for h in handles { h.join().unwrap(); }
    results
}

fn main() {
    // → (0..20).map(...).collect()：用迭代器构造任务向量。
    let jobs: Vec<Job> = (0..20).map(|i| Job {
        id: i,
        data: format!("task-{i}"),
    }).collect();

    let results = worker_pool(jobs, 4);
    for r in &results {
        println!("[worker {}] job {}: {}", r.worker_id, r.job_id, r.output);
    }
}
```

</details>

---

### 练习 4：高阶组合器管道 ★★（约 25 分钟）

创建一个 `Pipeline` 结构体来链式组合转换。它应支持 `.pipe(f)` 添加转换，以及 `.execute(input)` 运行整个链。

<details>
<summary>🔑 解答</summary>

```rust
struct Pipeline<T> {
    // → Vec<Box<dyn Fn(T) -> T>>：存储类型擦除的闭包集合。
    //   dyn Fn 是 trait 对象（动态分发），Box 将其放在堆上。
    //   Fn 表示不可变借用环境的闭包，可多次调用。
    transforms: Vec<Box<dyn Fn(T) -> T>>,
}

// → T: 'static 约束：要求 T 不含非 'static 引用，
//   因为 Box<dyn Fn> 默认 'static，闭包需能存活任意长。
impl<T: 'static> Pipeline<T> {
    fn new() -> Self {
        Pipeline { transforms: Vec::new() }
    }

    // → pipe(mut self, ...) -> Self：消耗并返回 self，实现链式调用（builder 风格）。
    //   impl Fn(T)->T + 'static：接受任意满足约束的闭包。
    fn pipe(mut self, f: impl Fn(T) -> T + 'static) -> Self {
        // → Box::new(f)：将闭包装箱为 trait 对象，统一类型存入 Vec。
        self.transforms.push(Box::new(f));
        self
    }

    fn execute(self, input: T) -> T {
        // → into_iter().fold(init, f)：fold 累积地应用每个转换，
        //   把上一步的输出作为下一步的输入，实现管道串联。
        //   into_iter 消耗 Vec 取得闭包所有权。
        self.transforms.into_iter().fold(input, |val, f| f(val))
    }
}

fn main() {
    // → 链式调用：每步 pipe 返回 Pipeline，最终 execute 触发执行。
    let result = Pipeline::new()
        .pipe(|s: String| s.trim().to_string())
        .pipe(|s| s.to_uppercase())
        .pipe(|s| format!(">>> {s} <<<"))
        .execute("  hello world  ".to_string());

    println!("{result}"); // >>> HELLO WORLD <<<

    // 数值管道：
    let result = Pipeline::new()
        .pipe(|x: i32| x * 2)
        .pipe(|x| x + 10)
        .pipe(|x| x * x)
        .execute(5);

    println!("{result}"); // (5*2 + 10)^2 = 400
}
```

**附加**：在阶段间改变类型的通用管道需要不同的设计 — 每个 `.pipe()` 返回一个具有不同输出类型的 `Pipeline`（这需要更高级的泛型管道工程）。

</details>

---

### 练习 5：使用 thiserror 的错误层级 ★★（约 30 分钟）

为一个文件处理应用设计错误类型层级，它可能在 I/O、解析（JSON 和 CSV）和校验时失败。使用 `thiserror` 并演示 `?` 传播。

<details>
<summary>🔑 解答</summary>

```rust,ignore
// → thiserror::Error：派生宏，为枚举自动实现 std::error::Error + Display。
//   #[error("...")] 指定 Display 文本，支持 {field} / {0} 格式化。
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    // → #[error("I/O error: {0}")]：{0} 引用元组变体的第 0 个字段。
    #[error("I/O error: {0}")]
    // → #[from]：自动实现 From<io::Error> for AppError，
    //   使 ? 能将 io::Error 转为 AppError::Io。
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    // → 命名字段变体：{line} {message} 按字段名格式化。
    #[error("CSV error at line {line}: {message}")]
    Csv { line: usize, message: String },

    #[error("validation error: {field} — {reason}")]
    Validation { field: String, reason: String },
}

fn read_file(path: &str) -> Result<String, AppError> {
    // → ? 触发 From 转换：io::Error 经 #[from] 变为 AppError::Io。
    Ok(std::fs::read_to_string(path)?) // io::Error → AppError::Io 通过 #[from]
}

fn parse_json(content: &str) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::from_str(content)?) // serde_json::Error → AppError::Json
}

fn validate_name(value: &serde_json::Value) -> Result<String, AppError> {
    // → serde_json::Value::get：按键索引，返回 Option<&Value>。
    // → and_then：链式 Option 处理，这里取出 str。
    // → as_str：将 Value 转为 Option<&str>（仅字符串值返回 Some）。
    let name = value.get("name")
        .and_then(|v| v.as_str())
        // → ok_or_else：Option → Result，None 时用闭包构造错误。
        .ok_or_else(|| AppError::Validation {
            field: "name".into(),
            reason: "must be a non-null string".into(),
        })?;

    if name.is_empty() {
        return Err(AppError::Validation {
            field: "name".into(),
            reason: "must not be empty".into(),
        });
    }

    Ok(name.to_string())
}

// → process_file 用 ? 串联三个可能失败的步骤，错误统一提升为 AppError。
fn process_file(path: &str) -> Result<String, AppError> {
    let content = read_file(path)?;
    let json = parse_json(&content)?;
    let name = validate_name(&json)?;
    Ok(name)
}

fn main() {
    // → match Result：显式处理成功与错误分支。
    match process_file("config.json") {
        Ok(name) => println!("Name: {name}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

</details>

---

### 练习 6：带关联类型的泛型 Trait ★★★（约 40 分钟）

设计一个 `Repository<T>` trait，带有关联的 `Error` 和 `Id` 类型。为内存存储实现它，并演示编译期类型安全。

<details>
<summary>🔑 解答</summary>

```rust
use std::collections::HashMap;

// → trait 带关联类型（associated types）：type Item/Id/Error。
//   关联类型在实现时才确定，使一个类型只能为 trait 提供一种实现
//   （区别于泛型参数可为同一类型提供多实现）。
trait Repository {
    type Item;
    type Id;
    type Error;

    // → 方法签名使用 Self::Item 等，引用实现时确定的类型。
    //   返回 Result<Option<&Self::Item>, Self::Error>：
    //   Option 表示可能不存在；& 借用避免 clone。
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
    // → HashMap<u64, User>：键值存储，u64 为 id。
    data: HashMap<u64, User>,
    next_id: u64,
}

impl InMemoryUserRepo {
    fn new() -> Self {
        InMemoryUserRepo { data: HashMap::new(), next_id: 1 }
    }
}

// 错误类型是 Infallible — 内存操作永不失败
// → std::convert::Infallible：永不发生的错误类型（空枚举）。
//   表示该实现操作绝不失败，Result<_, Infallible> 实质上总是 Ok。
impl Repository for InMemoryUserRepo {
    type Item = User;
    type Id = u64;
    type Error = std::convert::Infallible;

    // → HashMap::get：返回 Option<&V>，不存在返回 None。
    fn get(&self, id: &u64) -> Result<Option<&User>, Self::Error> {
        Ok(self.data.get(id))
    }

    fn insert(&mut self, item: User) -> Result<u64, Self::Error> {
        let id = self.next_id;
        self.next_id += 1;
        // → HashMap::insert：插入键值，返回 Option<V>（旧值，若覆盖）。
        self.data.insert(id, item);
        Ok(id)
    }

    fn delete(&mut self, id: &u64) -> Result<bool, Self::Error> {
        // → HashMap::remove：删除并返回旧值 Option<V>；
        //   is_some() 表示确实删除了某项。
        Ok(self.data.remove(id).is_some())
    }
}

// 泛型函数适用于任何 Repository：
// → <R: Repository>：R 是任何实现 Repository 的类型。
//   方法体内通过 R::Item、R::Id 引用关联类型。
fn create_and_fetch<R: Repository>(repo: &mut R, item: R::Item) -> Result<(), R::Error>
// → where 子句：为关联类型添加额外约束（需 Debug 才能用 {:?} 打印）。
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
    // → &mut repo：可变借用传入泛型函数；.into() 将 &str 转为 String。
    create_and_fetch(&mut repo, User {
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).unwrap();
}
```

</details>

---

### 练习 7：Unsafe 的安全封装（第 11 章）★★★（约 45 分钟）

编写一个 `FixedVec<T, const N: usize>` — 一个固定容量、栈分配的向量。
要求：
- `push(&mut self, value: T) -> Result<(), T>` 满时返回 `Err(value)`
- `pop(&mut self) -> Option<T>` 返回并移除最后一个元素
- `as_slice(&self) -> &[T]` 借用已初始化的元素
- 所有公共方法必须是安全的；所有 unsafe 必须用 `SAFETY:` 注释封装
- `Drop` 必须清理已初始化的元素

**提示**：使用 `MaybeUninit<T>` 和 `[const { MaybeUninit::uninit() }; N]`。

<details>
<summary>🔑 解答</summary>

```rust
// → std::mem::MaybeUninit<T>：表示"可能未初始化"的内存。
//   它是 unsafe 封装的基础原语：分配内存但不构造 T，
//   由调用方保证何时真正初始化，再 assume_init 取出。
use std::mem::MaybeUninit;

// → <T, const N: usize>：泛型类型参数 T + const 泛型 N（编译期常量）。
//   N 决定数组容量，实现栈分配的固定容量向量。
pub struct FixedVec<T, const N: usize> {
    // → [MaybeUninit<T>; N]：长度为 N 的数组，每个槽位可能未初始化。
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> FixedVec<T, N> {
    pub fn new() -> Self {
        FixedVec {
            // → [const { MaybeUninit::uninit() }; N]：内联 const 表达式
            //   初始化数组，每个元素都是未初始化的 MaybeUninit。
            data: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    // → push 返回 Result<(), T>：满时把值原样返回（Err(value)），不丢弃。
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.len >= N { return Err(value); }
        // SAFETY: len < N，所以 data[len] 在边界内。
        // → MaybeUninit::new(value)：构造一个已初始化的 MaybeUninit。
        self.data[self.len] = MaybeUninit::new(value);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 { return None; }
        self.len -= 1;
        // SAFETY: data[len] 已初始化（自减前 len > 0）。
        // → unsafe { assume_init_read() }：读取并**移动**出值
        //   （不置空内存，调用方负责不再访问该槽位）。返回 T。
        Some(unsafe { self.data[self.len].assume_init_read() })
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: data[0..len] 都已初始化，且 MaybeUninit<T>
        // 与 T 有相同的内存布局。
        // → std::slice::from_raw_parts：从裸指针+长度构造切片。
        //   as_ptr 取数组首指针，as *const T 转换指针类型。
        //   这是 unsafe，因为需保证指针有效且内存已初始化。
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const T, self.len) }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

impl<T, const N: usize> Drop for FixedVec<T, N> {
    fn drop(&mut self) {
        // SAFETY: data[0..len] 已初始化 — 逐个 drop。
        // → Drop 中必须 drop 已初始化的元素，否则内存泄漏。
        //   assume_init_drop：原地析构已初始化值，不移动。
        for i in 0..self.len {
            unsafe { self.data[i].assume_init_drop(); }
        }
    }
}

fn main() {
    // → FixedVec::<String, 4>：turbofish 指定 T=String, N=4。
    let mut v = FixedVec::<String, 4>::new();
    v.push("hello".into()).unwrap();
    v.push("world".into()).unwrap();
    assert_eq!(v.as_slice(), &["hello", "world"]);
    assert_eq!(v.pop(), Some("world".into()));
    assert_eq!(v.len(), 1);
    // Drop 清理剩余的 "hello"
}
```

</details>

---

### 练习 8：声明式宏 — `map!`（第 12 章）★（约 15 分钟）

编写一个 `map!` 宏，从键值对创建 `HashMap`，类似 `vec![]`：

```rust
// → 演示目标：map! 宏应像 vec! 一样从键值对创建 HashMap。
let m = map! {
    "host" => "localhost",
    "port" => "8080",
};
// → HashMap::get(&k)：返回 Option<&V>，键不存在返回 None。
assert_eq!(m.get("host"), Some(&"localhost"));
// → HashMap::len：返回键值对数量。
assert_eq!(m.len(), 2);
```

要求：
- 支持尾随逗号
- 支持空调用 `map!{}`
- 对任何实现了 `Into<K>` 和 `Into<V>` 的类型都能工作，以达到最大灵活性

<details>
<summary>🔑 解答</summary>

```rust
// → macro_rules!：声明式宏定义。通过模式匹配宏调用语法并展开为代码。
//   比 fn 强大：能在编译期生成重复代码、接受可变参数。
macro_rules! map {
    // 空的情况
    // → () => {...}：匹配空调用，展开为空 HashMap。
    () => {
        std::collections::HashMap::new()
    };
    // 一个或多个 key => value 对（尾随逗号可选）
    // → $( ... ),+ ：重复元语法。$key/$val 是元变量，,+ 表示一个或多个、用逗号分隔。
    //   $(,)? 匹配可选的尾随逗号。
    ( $( $key:expr => $val:expr ),+ $(,)? ) => {{
        let mut m = std::collections::HashMap::new();
        // → $( ... )+ ：对每个匹配的 key/val 对重复展开 insert 语句。
        $( m.insert($key, $val); )+
        m
    }};
}

fn main() {
    // 基本用法：
    let config = map! {
        "host" => "localhost",
        "port" => "8080",
        "timeout" => "30",
    };
    assert_eq!(config.len(), 3);
    // → HashMap 实现了 Index，可用 m[k] 索引（键不存在时 panic）。
    assert_eq!(config["host"], "localhost");

    // 空 map：
    let empty: std::collections::HashMap<String, String> = map!();
    assert!(empty.is_empty());

    // 不同类型：
    let scores = map! {
        1 => 100,
        2 => 200,
    };
    assert_eq!(scores[&1], 100);
}
```

</details>

---

### 练习 9：自定义 serde 反序列化（第 10 章）★★★（约 45 分钟）

设计一个 `Duration` 封装类型，通过自定义 serde 反序列化器从人类可读的字符串（如 `"30s"`、`"5m"`、`"2h"`）反序列化。该结构体还应能序列化回相同格式。

<details>
<summary>🔑 解答</summary>

```rust,ignore
// → serde 的核心 trait：Serialize/Deserialize。
//   手动实现它们可完全控制序列化/反序列化逻辑（这里：字符串 ↔ Duration）。
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
// → HumanDuration(std::time::Duration)：新类型包装标准 Duration。
struct HumanDuration(std::time::Duration);

impl HumanDuration {
    // → from_str：自定义解析逻辑，返回 Result<Self, String>。
    fn from_str(s: &str) -> Result<Self, String> {
        // → str::trim：去除首尾空白。
        let s = s.trim();
        if s.is_empty() { return Err("empty duration string".into()); }

        // → str::find(|c: char| !c.is_ascii_digit())：找到第一个非数字字符位置。
        //   split_at(idx)：在此处切分，得到 (数字部分, 后缀部分)。
        //   unwrap_or(s.len()) 表示全是数字时后缀为空。
        let (num_str, suffix) = s.split_at(
            s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len())
        );
        // → str::parse::<u64>：解析数字部分。
        //   map_err 将 ParseIntError 转为带上下文的字符串错误。
        let value: u64 = num_str.parse()
            .map_err(|_| format!("invalid number: {num_str}"))?;

        // → 根据后缀选择 Duration 构造方法。
        //   Duration::from_secs / from_millis：从秒/毫秒构造 Duration。
        let duration = match suffix {
            "s" | "sec"  => std::time::Duration::from_secs(value),
            "m" | "min"  => std::time::Duration::from_secs(value * 60),
            "h" | "hr"   => std::time::Duration::from_secs(value * 3600),
            "ms"         => std::time::Duration::from_millis(value),
            other        => return Err(format!("unknown suffix: {other}")),
        };
        Ok(HumanDuration(duration))
    }
}

// → 实现 Display 使 HumanDuration 可格式化为人类可读字符串（序列化时复用）。
impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // → Duration::as_secs / as_millis：取秒/毫秒表示（截断）。
        let secs = self.0.as_secs();
        if secs == 0 {
            write!(f, "{}ms", self.0.as_millis())
        } else if secs % 3600 == 0 {
            write!(f, "{}h", secs / 3600)
        } else if secs % 60 == 0 {
            write!(f, "{}m", secs / 60)
        } else {
            write!(f, "{}s", secs)
        }
    }
}

// → 手动实现 Serialize：序列化为字符串形式。
impl Serialize for HumanDuration {
    // → 泛型 S: Serializer，关联类型通过 S::Ok/S::Error 引用。
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // → Serializer::serialize_str：将值写为字符串。
        serializer.serialize_str(&self.to_string())
    }
}

// → 'de 生命周期：Deserializer 绑定的反序列化生命周期。
impl<'de> Deserialize<'de> for HumanDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // → 先把输入反序列化为 String，再用自定义逻辑解析。
        let s = String::deserialize(deserializer)?;
        // → serde::de::Error::custom：将任意错误转为 serde 的反序列化错误类型。
        HumanDuration::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// → #[derive(Deserialize, Serialize)]：派生宏自动为字段类型实现 serde trait，
//   HumanDuration 因已手动实现而被自动复用。
#[derive(Debug, Deserialize, Serialize)]
struct Config {
    timeout: HumanDuration,
    retry_interval: HumanDuration,
}

fn main() {
    let json = r#"{ "timeout": "30s", "retry_interval": "5m" }"#;
    // → serde_json::from_str：反序列化 JSON，自动调用 HumanDuration::deserialize。
    let config: Config = serde_json::from_str(json).unwrap();

    assert_eq!(config.timeout.0, std::time::Duration::from_secs(30));
    assert_eq!(config.retry_interval.0, std::time::Duration::from_secs(300));

    // 正确地往返：
    // → serde_json::to_string：序列化，自动调用 HumanDuration::serialize。
    let serialized = serde_json::to_string(&config).unwrap();
    assert!(serialized.contains("30s"));
    assert!(serialized.contains("5m"));
    println!("Config: {serialized}");
}
```

</details>

### 练习 10 — 带超时的并发抓取器 ★★（约 25 分钟）

编写一个 async 函数 `fetch_all`，它派生三个 `tokio::spawn` 任务，每个任务
用 `tokio::time::sleep` 模拟网络调用。用 `tokio::try_join!` 连接三者，
并包裹在 `tokio::time::timeout(Duration::from_secs(5), ...)` 中。
返回 `Result<Vec<String>, ...>`，如果任何任务失败或截止时间过期则返回错误。

**学习目标**：`tokio::spawn`、`try_join!`、`timeout`、跨任务边界的错误传播。

<details>
<summary>提示</summary>

每个派生的任务返回 `Result<String, _>`。`try_join!` 解包三者。
将整个 `try_join!` 包裹在 `timeout()` 中 — `Elapsed` 错误意味着你触碰了截止时间。

</details>

<details>
<summary>解答</summary>

```rust,ignore
// → tokio 异步时间 API：sleep（异步休眠）、timeout（截止时间包装）、Duration。
use tokio::time::{sleep, timeout, Duration};

async fn fake_fetch(name: &'static str, delay_ms: u64) -> Result<String, String> {
    sleep(Duration::from_millis(delay_ms)).await;
    Ok(format!("{name}: OK"))
}

async fn fetch_all() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let deadline = Duration::from_secs(5);

    // → timeout(deadline, future)：超时则返回 Err(Elapsed)，future 被 drop。
    let (a, b, c) = timeout(deadline, async {
        // → tokio::spawn：派生并发任务，返回 JoinHandle。
        let h1 = tokio::spawn(fake_fetch("svc-a", 100));
        let h2 = tokio::spawn(fake_fetch("svc-b", 200));
        let h3 = tokio::spawn(fake_fetch("svc-c", 150));
        // → try_join!：并发等待，任一 Err 立即短路。
        tokio::try_join!(h1, h2, h3)
    })
    // → 第一个 ? = timeout（Elapsed），第二个 ? = try_join（任务错误）。
    .await??;

    // → a?, b?, c?：解包 JoinHandle（任务 panic 时 Err）+ 内部 fake_fetch 的 Result。
    Ok(vec![a?, b?, c?]) // 解包内部 Result
}

#[tokio::main]
async fn main() {
    let results = fetch_all().await.unwrap();
    for r in &results {
        println!("{r}");
    }
}
```

</details>

### 练习 11 — Async 通道管道 ★★★（约 40 分钟）

使用 `tokio::sync::mpsc` 构建一个 生产者 → 转换器 → 消费者 的管道：

1. **生产者**：将整数 1..=20 发入通道 A（容量 4）。
2. **转换器**：从通道 A 读取，将每个值平方，发入通道 B。
3. **消费者**：从通道 B 读取，收集到 `Vec<u64>`，返回它。

三个阶段都作为并发的 `tokio::spawn` 任务运行。使用有界通道来
演示背压（back-pressure）。断言最终 vec 等于 `[1, 4, 9, ..., 400]`。

**学习目标**：`mpsc::channel`、有界背压、带 move 闭包的 `tokio::spawn`、
通过通道关闭实现的优雅关闭。

<details>
<summary>解答</summary>

```rust,ignore
// → tokio::sync::mpsc：异步多生产者单消费者通道。
//   与 std::sync::mpsc 不同，其 send/recv 是 async，可配合 .await。
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // → mpsc::channel(capacity)：创建有界通道，容量满时 send 会 await（背压）。
    //   返回 (Sender, Receiver)。Receiver 用 mut 因 recv 需可变借用。
    //   有界容量 4 实现"背压"：生产者不会无限堆积数据。
    let (tx_a, mut rx_a) = mpsc::channel::<u64>(4); // 有界 — 背压
    let (tx_b, mut rx_b) = mpsc::channel::<u64>(4);

    // 生产者
    // → tokio::spawn + move：把 tx_a 移入异步任务。
    let producer = tokio::spawn(async move {
        for i in 1..=20u64 {
            // → Sender::send：异步发送，缓冲满时 await 等待空间（背压生效）。
            tx_a.send(i).await.unwrap();
        }
        // tx_a 在此丢弃 → 通道 A 关闭
        // → tx_a drop 后，下游 rx_a.recv() 将返回 None，触发转换器退出。
    });

    // 转换器
    let transformer = tokio::spawn(async move {
        // → while let Some(val) = rx_a.recv().await：循环接收，
        //   recv 返回 Option<T>，None 表示所有 Sender 已 drop（通道关闭）。
        while let Some(val) = rx_a.recv().await {
            tx_b.send(val * val).await.unwrap();
        }
        // tx_b 在此丢弃 → 通道 B 关闭
    });

    // 消费者
    let consumer = tokio::spawn(async move {
        let mut results = Vec::new();
        while let Some(val) = rx_b.recv().await {
            results.push(val);
        }
        results
    });

    // → JoinHandle::await：等待任务完成，返回 Result<T, JoinError>。
    producer.await.unwrap();
    transformer.await.unwrap();
    // consumer 返回的 Vec 通过 await 取回主任务。
    let results = consumer.await.unwrap();

    let expected: Vec<u64> = (1..=20).map(|x: u64| x * x).collect();
    assert_eq!(results, expected);
    println!("Pipeline complete: {results:?}");
}
```

</details>

***
