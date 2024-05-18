# Tutorial

Tokio是Rust的异步运行时。提供了编写网络应用所需要的构建模块。它可以灵活地针对各种系统，从具有数十个内核的大型服务器到小型嵌入式设备。

从高层次上看，Tokio提供了以下主要模块：

- 一个执行异步代码的多线程运行时。
- 一个标准库的异步版本。
- 一个庞大的库生态系统。

# Tokio在你的项目中扮演的角色

当用异步方式编写程序时，可以通过降低同时执行许多操作的成本，来让其能更好的扩展。然而，异步的Rust代码不能自动运行，所以你必须用一个运行时来执行，Tokio就是最广泛使用的运行时，其使用量超过了其他运行时的总和。

此外，Tokio也提供了很多好用的工具。当编写异步代码时，你不能使用Rust标准库提供的普通阻塞API，你必须用它们的异步版本。这些替代版本将会由Tokio提供，在有意义的(make sense)地方反映了Rust标准库的API。

# Tokio的优势

本节将会列出Tokio的一些优势。

## 快速

Tokio是非常*快速*的，基于本身就很快的Rust构建，这是本着Rust的精神完成的，其目标是您不用通过手动编写等效的代码来提高性能。

Tokio是可扩展的，建立在async/await语言功能(feature)之上，而async/await语言功能本身是可扩展的。在处理网络时，由于延迟，处理连接的速度是有限制的，因此唯一的扩展方法是一次处理多个连接。借助async/await语言功能，增加并发操作的数量变得非常容易，这将允许扩展到大量的并发任务。

## 可靠

Tokio使用Rust构建，Rust是一个可以使每个人都能构建可靠且高效的软件的语言。[许多](https://www.zdnet.com/article/microsoft-70-percent-of-all-security-bugs-are-memory-safety-issues/)的[研究](https://www.chromium.org/Home/chromium-security/memory-safety)发现，大约 ~70% 的高严重性安全漏洞是内存不安全的结果。而使用Rust可以消除这些应用程序中所有的该类错误。

Tokio还非常注重提供一致性的行为，同样的代码不会导致其他结果。Tokio的主要目标就是让用户编写可以预测行为的软件，该软件日复一夜地执行并且能够具有可靠的响应时间，不会出现不可预测的延迟峰值。

## 易用

借助Rust的async/await功能，编写异步程序的复杂性大大降低。与Tokio工具和充满活力的生态系统相结合，编写程序将会变得轻而易举。

Tokio在合理的情况下(make sense)遵守标准库的命名约定。这使得可以轻松地将使用标准库写的代码转换为使用Tokio编写的代码。通过Rust强大的类型系统，轻松交付正确代码的能力将得到很大提升。

## 灵活

Tokio提供了多种运行时变体。从多线程、[work-stealing](https://en.wikipedia.org/wiki/Work_stealing)的运行时到轻量级的单线程运行时，应有尽有。这些运行时中的每一个都带有许多选项，允许用户根据自己的需要调整它们。

# 不该用Tokio的场景

纵使Tokio对于很多需要同时执行大量操作的项目很有用，但是也有一些情况不太适合使用Tokio。

- 在多个线程上并行运行CPU密集型运算任务。Tokio是专为IO密集型应用来设计的，而且其中单独任务大部分时间都在等待IO。如果您的应用只做并行计算，那么您应该使用[rayon](https://docs.rs/rayon/)。也就是说，仍然可以“混合搭配”，如果您两件事都要做的话。请看[这篇文章了解实例](https://ryhl.io/blog/async-what-is-blocking/#the-rayon-crate)。
- 读取大量文件。虽然Tokio似乎对于只需要读取大量文件的项目很有用，但是与普通线程池相比，Tokio在这种情况没有任何优势。这是因为操作系统一般不提供异步文件API。
- 发送单个Web请求。Tokio的优势在于在同时做很多事情。如果需要使用用于异步Rust的库，例如[reqwest](https://docs.rs/reqwest/)，但是不需要同时做很多事，可以选择该库的阻塞版本，这样会使得项目更简单。当然，使用Tokio也行，但是与阻塞API相比没有真正的优势。如果该库不提供阻塞API，请看[有关于桥接同步代码的章节](https://tokio.rs/tokio/topics/bridging)。

# 获取帮助

如果您在任何时候遇到困难，都可以在[Discord](https://discord.gg/tokio)或者[Github discussions](https://github.com/tokio-rs/tokio/discussions)中获得帮助。不要羞于提问“初学者”问题，我们随时准备着乐于提供帮助