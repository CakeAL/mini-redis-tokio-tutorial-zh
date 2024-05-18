# Setup 准备工作

本教程将会带您完整构建[Redis](https://redis.io/)客户端(client)和服务端(server)的过程。我们将从异步 Rust 的基础知识开始，并从这里开始构建。我们将会实现 Redis 命令的子集，但同时会对 Tokio 进行全面介绍。

# Mini-Redis

本教程构建的项目[Mini-Redis 在 GitHub 上](https://github.com/tokio-rs/mini-redis)。Mini-Redis 被设计于学习 Tokio，广受好评，但是同时也缺失了一些真正 Redis 中的功能。当然您可以在[crate.io](https://crates.io/)上找到可用于生产的 Redis 库。

我们将会在教程中直接使用 Mini-Redis。这让我们可以先使用 Mini-Redis 中的部分内容，之后再在本教程后面实现它们。

# 获取帮助

如果您在任何时候遇到困难，都可以在[Discord](https://discord.gg/tokio)或者[Github discussions](https://github.com/tokio-rs/tokio/discussions)中获得帮助。不要羞于提问“初学者”问题，我们随时准备着乐于提供帮助

# 预先准备

读者应该已经熟悉[Rust](https://rust-lang.org/)，[《Rust book》](https://doc.rust-lang.org/book/)是一本极好的入门资源。

虽然不是必须的，但是有使用[Rust 标准库](https://doc.rust-lang.org/std/)和其他语言网络编程的经验会有一些帮助。

无需提前知道关于 Redis 的知识。

## Rust

在开始之前，你需要确保[Rust](https://www.rust-lang.org/tools/install)工具链已经安装并随时可用。如果没装，使用[rustup](https://rustup.rs/)安装时最容易的。

本教程需要 Rust 最低版本是`1.45.0`，但是用最新稳定版是推荐的。

检查电脑上 Rust 版本，在终端中运行：

```bash
$ rustc --version
```

您应该看到这样的输出`rustc 1.78.0 (9b00956e5 2024-04-29)`

## Mini-Redis 服务端

接下来，安装 Mini-Redis 服务端。这个可以用来测试我们构建的客户端。

```bash
$ cargo install mini-redis
```

通过运行服务端来确保安装成功：

```bash
$ mini-redis-server
```

然后，在另外一个终端窗口，尝试使用`mini-redis-cli`获取 key `foo`

```bash
$ mini-redis-cli get foo
```

你会看到`(nil)`。

# 准备好开始

就这样一切准备就绪了，转到下一页来编写您的第一个异步 Rust 应用程序。
