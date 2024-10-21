# Tutorial

Tokio 是 Rust 的异步运行时。提供了编写网络应用所需要的构建模块。它可以灵活地针对各种系统，从具有数十个内核的大型服务器到小型嵌入式设备。

从高层次上看，Tokio 提供了以下主要模块：

- 一个执行异步代码的多线程运行时。
- 一个标准库的异步版本。
- 一个庞大的库生态系统。

# Tokio 在你的项目中扮演的角色

当用异步方式编写程序时，可以通过降低同时执行许多操作的成本，来让其能更好的扩展。然而，异步的 Rust 代码不能自动运行，所以你必须用一个运行时来执行，Tokio 就是最广泛使用的运行时，其使用量超过了其他运行时的总和。

此外，Tokio 也提供了很多好用的工具。当编写异步代码时，你不能使用 Rust 标准库提供的普通阻塞 API，你必须用它们的异步版本。这些替代版本将会由 Tokio 提供，在有意义的(make sense)地方反映了 Rust 标准库的 API。

# Tokio 的优势

本节将会列出 Tokio 的一些优势。

## 快速

Tokio 是非常*快速*的，基于本身就很快的 Rust 构建，这是本着 Rust 的精神完成的，其目标是您不用通过手动编写等效的代码来提高性能。

Tokio 是可扩展的，建立在 async/await 语言功能(feature)之上，而 async/await 语言功能本身是可扩展的。在处理网络时，由于延迟，处理连接的速度是有限制的，因此唯一的扩展方法是一次处理多个连接。借助 async/await 语言功能，增加并发操作的数量变得非常容易，这将允许扩展到大量的并发任务。

## 可靠

Tokio 使用 Rust 构建，Rust 是一个可以使每个人都能构建可靠且高效的软件的语言。[许多](https://www.zdnet.com/article/microsoft-70-percent-of-all-security-bugs-are-memory-safety-issues/)的[研究](https://www.chromium.org/Home/chromium-security/memory-safety)发现，大约 ~70% 的高严重性安全漏洞是内存不安全的结果。而使用 Rust 可以消除这些应用程序中所有的该类错误。

Tokio 还非常注重提供一致性的行为，同样的代码不会导致其他结果。Tokio 的主要目标就是让用户编写可以预测行为的软件，该软件日复一夜地执行并且能够具有可靠的响应时间，不会出现不可预测的延迟峰值。

## 易用

借助 Rust 的 async/await 功能，编写异步程序的复杂性大大降低。与 Tokio 工具和充满活力的生态系统相结合，编写程序将会变得轻而易举。

Tokio 在合理的情况下(make sense)遵守标准库的命名约定。这使得可以轻松地将使用标准库写的代码转换为使用 Tokio 编写的代码。通过 Rust 强大的类型系统，轻松交付正确代码的能力将得到很大提升。

## 灵活

Tokio 提供了多种运行时变体。从多线程、[work-stealing](https://en.wikipedia.org/wiki/Work_stealing)的运行时到轻量级的单线程运行时，应有尽有。这些运行时中的每一个都带有许多选项，允许用户根据自己的需要调整它们。

# 不该用 Tokio 的场景

纵使 Tokio 对于很多需要同时执行大量操作的项目很有用，但是也有一些情况不太适合使用 Tokio。

- 在多个线程上并行运行 CPU 密集型运算任务。Tokio 是专为 IO 密集型应用来设计的，而且其中单独任务大部分时间都在等待 IO。如果您的应用只做并行计算，那么您应该使用[rayon](https://docs.rs/rayon/)。也就是说，仍然可以“混合搭配”，如果您两件事都要做的话。请看[这篇文章了解实例](https://ryhl.io/blog/async-what-is-blocking/#the-rayon-crate)。
- 读取大量文件。虽然 Tokio 似乎对于只需要读取大量文件的项目很有用，但是与普通线程池相比，Tokio 在这种情况没有任何优势。这是因为操作系统一般不提供异步文件 API。
- 发送单个 Web 请求。Tokio 的优势在于在同时做很多事情。如果需要使用用于异步 Rust 的库，例如[reqwest](https://docs.rs/reqwest/)，但是不需要同时做很多事，可以选择该库的阻塞版本，这样会使得项目更简单。当然，使用 Tokio 也行，但是与阻塞 API 相比没有真正的优势。如果该库不提供阻塞 API，请看[有关于桥接同步代码的章节](https://tokio.rs/tokio/topics/bridging)。

# 获取帮助

如果您在任何时候遇到困难，都可以在[Discord](https://discord.gg/tokio)或者[Github discussions](https://github.com/tokio-rs/tokio/discussions)中获得帮助。不要羞于提问“初学者”问题，我们随时准备着乐于提供帮助
