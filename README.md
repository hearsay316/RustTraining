<div style="background-color: #d9d9d9; padding: 16px; border-radius: 6px; color: #000000;">

**License / 许可证** This project is dual-licensed under the [MIT License](LICENSE) and [Creative Commons Attribution 4.0 International (CC-BY-4.0)](LICENSE-DOCS).  
本项目采用双许可证：[MIT License](LICENSE) 和 [Creative Commons Attribution 4.0 International (CC-BY-4.0)](LICENSE-DOCS)。

</div>

<div style="background-color: #d9d9d9; padding: 16px; border-radius: 6px; color: #000000;">

**Trademarks / 商标** This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft trademarks or logos is subject to and must follow [Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general). Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship. Any use of third-party trademarks or logos are subject to those third-party's policies.  
本项目可能包含相关项目、产品或服务的商标或徽标。Microsoft 商标或徽标的授权使用须遵守 [Microsoft 商标与品牌指南](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general)。在本项目的修改版本中使用 Microsoft 商标或徽标，不得造成混淆或暗示 Microsoft 赞助。任何第三方商标或徽标的使用均须遵守相应第三方的政策。

</div>

# Rust Training Books / Rust 培训书籍

Seven training courses covering Rust from different programming backgrounds, plus deep-dives on async, advanced patterns, and engineering practices.  
七门培训课程，从不同编程背景出发介绍 Rust，并深入讲解异步编程、高级模式和工程实践。

This material combines original content with ideas and examples inspired by some of the best resources in the Rust ecosystem. The goal is to present an in-depth, technically accurate curriculum that weaves together knowledge scattered across books, blogs, conference talks, and video series into a cohesive, pedagogically structured experience.  
本资料将原创内容与 Rust 生态中一些优秀资源启发而来的思想和示例相结合。目标是提供一套深入、技术准确的课程体系，将分散在书籍、博客、会议演讲和视频系列中的知识整合为连贯且具有教学结构的学习体验。

> **Disclaimer / 免责声明：** These books are training material, not an authoritative reference. While we strive for accuracy, always verify critical details against the [official Rust documentation](https://doc.rust-lang.org/) and the [Rust Reference](https://doc.rust-lang.org/reference/).  
> 这些书籍是培训资料，并非权威参考。虽然我们努力确保准确性，但关键细节仍应以 [Rust 官方文档](https://doc.rust-lang.org/) 和 [Rust Reference](https://doc.rust-lang.org/reference/) 为准。

### Inspirations & Acknowledgments / 灵感来源与致谢

- [**The Rust Programming Language**](https://doc.rust-lang.org/book/) — the foundation everything builds on  
  一切内容构建其上的基础
- [**Jon Gjengset**](https://www.youtube.com/c/JonGjengset) — deep-dive streams on advanced Rust internals, `Crust of Rust` series  
  关于 Rust 高级内部机制的深度直播，以及 `Crust of Rust` 系列
- [**withoutboats**](https://without.boats/blog/) — async design, `Pin`, and the futures model  
  异步设计、`Pin` 和 futures 模型
- [**fasterthanlime (Amos)**](https://fasterthanli.me/) — systems programming from first principles, engaging long-form explorations  
  从第一性原理出发的系统编程，以及引人入胜的长篇探索
- [**Mara Bos**](https://marabos.nl/) — *Rust Atomics and Locks*, concurrency primitives  
  *Rust Atomics and Locks*，并发原语
- [**Aleksey Kladov (matklad)**](https://matklad.github.io/) — Rust analyzer insights, API design, error handling patterns  
  Rust analyzer 洞见、API 设计、错误处理模式
- [**Niko Matsakis**](https://smallcultfollowing.com/babysteps/) — language design, borrow checker internals, Polonius  
  语言设计、借用检查器内部机制、Polonius
- [**Rust by Example**](https://doc.rust-lang.org/rust-by-example/) and [**Rustonomicon**](https://doc.rust-lang.org/nomicon/) — practical patterns and unsafe deep-dives  
  实用模式与 unsafe 深入讲解
- [**This Week in Rust**](https://this-week-in-rust.org/) — community discoveries that shaped many examples  
  Rust 社区发现，塑造了许多示例
- [**Binary Musings - Tag(Rust)**](https://binarymusings.org/posts/category/rust/) — Deep dive into Rust internals  
  深入解析 Rust 内部机制
- …and many others in the **Rust community at large** whose blog posts, conference talks, RFCs, and forum discussions have informed this material — too numerous to list individually, but deeply appreciated  
  ……以及更广泛 **Rust 社区** 中的许多贡献者，他们的博客文章、会议演讲、RFC 和论坛讨论为本资料提供了启发。人数众多，无法一一列举，但我们深表感谢

## 📖 Start Reading / 开始阅读

Pick the book that matches your background. Books are grouped by complexity so you can chart a learning path:  
选择与你背景匹配的书籍。书籍按复杂度分组，便于规划学习路径：

| Level / 级别 | Description / 说明 |
|-------|-------------|
| 🟢 **Bridge / 桥接** | Learn Rust coming from another language — start here<br>从另一门语言转向 Rust —— 建议从这里开始 |
| 🔵 **Deep Dive / 深入探索** | Focused exploration of a major Rust subsystem<br>聚焦 Rust 主要子系统的专题探索 |
| 🟡 **Advanced / 高级** | Patterns and techniques for experienced Rustaceans<br>面向有经验 Rustacean 的模式与技术 |
| 🟣 **Expert / 专家** | Cutting-edge type-level and correctness techniques<br>前沿的类型级与正确性技术 |
| 🟤 **Practices / 实践** | Engineering, tooling, and production readiness<br>工程、工具链与生产就绪能力 |

| Book / 书籍 | Level / 级别 | Who it's for / 适合人群 |
|------|-------|-------------|
| [**Rust for C/C++ Programmers**](https://microsoft.github.io/RustTraining/c-cpp-book/) | 🟢 Bridge / 桥接 | Move semantics, RAII, FFI, embedded, no_std<br>移动语义、RAII、FFI、嵌入式、no_std |
| [**Rust for C# Programmers**](https://microsoft.github.io/RustTraining/csharp-book/) | 🟢 Bridge / 桥接 | Swift / C# / Java → ownership & type system<br>Swift / C# / Java → 所有权与类型系统 |
| [**Rust for Python Programmers**](https://microsoft.github.io/RustTraining/python-book/) | 🟢 Bridge / 桥接 | Dynamic → static typing, GIL-free concurrency<br>动态类型 → 静态类型、无 GIL 并发 |
| [**Async Rust**](https://microsoft.github.io/RustTraining/async-book/) / [中文](https://microsoft.github.io/RustTraining/async-book/zh/) | 🔵 Deep Dive / 深入探索 | Tokio, streams, cancellation safety<br>Tokio、流、取消安全 |
| [**Rust Patterns**](https://microsoft.github.io/RustTraining/rust-patterns-book/) | 🟡 Advanced / 高级 | Pin, allocators, lock-free structures, unsafe<br>Pin、分配器、无锁结构、unsafe |
| [**Type-Driven Correctness**](https://microsoft.github.io/RustTraining/type-driven-correctness-book/) | 🟣 Expert / 专家 | Type-state, phantom types, capability tokens<br>类型状态、幻影类型、能力令牌 |
| [**Rust Engineering Practices**](https://microsoft.github.io/RustTraining/engineering-book/) | 🟤 Practices / 实践 | Build scripts, cross-compilation, CI/CD, Miri<br>构建脚本、交叉编译、CI/CD、Miri |

Each book has 15–16 chapters with Mermaid diagrams, editable Rust playgrounds, exercises, and full-text search.  
每本书包含 15–16 章，提供 Mermaid 图表、可编辑的 Rust playground、练习和全文搜索。

> **Tip / 提示：** Browse the rendered books with sidebar navigation and search at the [GitHub Pages site](https://microsoft.github.io/RustTraining/).  
> 可在 [GitHub Pages 站点](https://microsoft.github.io/RustTraining/) 使用侧边栏导航和搜索浏览渲染后的书籍。
>
> **Local preview / 本地预览：** For offline reading or while contributing ([install Rust](https://rustup.rs/) first):  
> 用于离线阅读或参与贡献（请先[安装 Rust](https://rustup.rs/)）：
> ```
> git clone https://github.com/microsoft/RustTraining.git
> cd RustTraining
> cargo install mdbook mdbook-mermaid
> cargo xtask serve    # http://localhost:3000
> ```

---

## 🔧 For Maintainers / 维护者指南

<details>
<summary>Building, serving, and editing the books locally / 在本地构建、运行和编辑书籍</summary>

### Prerequisites / 前置条件

Install [Rust via **rustup**](https://rustup.rs/) if you haven't already, then:  
如果尚未安装，请先通过 [**rustup** 安装 Rust](https://rustup.rs/)，然后执行：

```bash
cargo install mdbook@0.4.52 mdbook-mermaid@0.14.0
```

### Clone the repo / 克隆仓库

```bash
git clone https://github.com/microsoft/RustTraining.git
cd RustTraining
```

### Build & serve / 构建与运行

```bash
cargo xtask build               # Build all books into site/ (local preview) / 将所有书籍构建到 site/（本地预览）
cargo xtask serve               # Build and serve at http://localhost:3000 / 构建并在 http://localhost:3000 提供服务
cargo xtask deploy              # Build all books into docs/ (for GitHub Pages) / 将所有书籍构建到 docs/（用于 GitHub Pages）
cargo xtask clean               # Remove site/ and docs/ / 删除 site/ 和 docs/
```

To build or serve a single book:  
构建或运行单本书：

```bash
cd c-cpp-book && mdbook serve --open    # http://localhost:3000
```

### Deployment / 部署

The site auto-deploys to GitHub Pages on push to `main` via `.github/workflows/pages.yml`. No manual steps needed.  
站点会在推送到 `main` 后，通过 `.github/workflows/pages.yml` 自动部署到 GitHub Pages，无需手动操作。

</details>



