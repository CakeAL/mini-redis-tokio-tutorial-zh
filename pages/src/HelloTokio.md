# Hello Tokio 你好 Tokio

我们将会从编写一个非常基础的 Tokio 应用开始。它将会连接到 Mini-Redis 服务器，把键(key) `hello` 的值(value)设置为`world`。然后读取 key。这将使用 Mini-Redis 客户端库(client library)来完成。

# 代码

## 建一个新 crate

让我们通过新建一个 Rust 应用开始：

```bash
$ cargo new my-redis
$ cd my-redis
```

## 添加依赖

接下来，打开`Cargo.toml`并且添加如下依赖到`[dependencies]`：

```toml
tokio = { version = "1", features = ["full"] }
mini-redis = "0.4"
```

## 编写代码

然后，打开`main.rs`，替换内容如下：

```rust
use mini_redis::{client, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // 对mini-redis的地址打开一个链接
    let mut client = client::connect("127.0.0.1:6379").await?;

    // 设置key "hello" 的value为 "world"
    client.set("hello", "world".into()).await?;

    // 获取key "hello"
    let result = client.get("hello").await?;

    println!("got value from the server; result={:?}", result);

    Ok(())
}
```

确保 Mini-Redis 服务端正在运行。新建一个终端窗口，执行：

```bash
$ mini-redis-server
```

如果你还没安装 mini-redis，这样做：

```bash
$ cargo install mini-redis
```

现在，运行`my-redis`应用：

```bash
$ cargo run
got value from the server; result=Some(b"world")
```

成功了！ \
你可以在[这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/hello-tokio/src/main.rs)找到全部代码。

# 代码分解

让我们花点时间看看我们刚刚干了什么。虽然没有多少代码，但是却发生了很多事。

```rust
let mut client = client::connect("127.0.0.1:6379").await?;
```

[`client::connect`](https://docs.rs/mini-redis/0.4/mini_redis/client/fn.connect.html)函数是`mini-redis`crate 提供的。它与指定的远程地址异步建立 TCP 连接。连接建立后，`client`句柄(handle)被返回。虽然操作是异步执行的，但是我们编写的代码却看起来是同步的。该操作是异步的唯一提示就是`.await`操作符。

## 什么是异步编程？

大多数计算机程序的执行顺序与编写顺序相同。第一行执行，然后执行下一行，依此类推。同步编程时，当程序遇到无法立即完成的操作时，就会阻塞，直到操作完成。例如，建立 TCP 连接需要通过网络与对等方进行交换，这可能需要相当长的时间。在此期间，线程被阻塞。

使用异步编程时，无法立即完成的操作将在后台挂起。线程不会被阻塞，并且可以继续运行其他内容。操作完成后，任务将取消挂起，并从中断的位置继续处理。我们之前的示例只有一个任务，因此在挂起时不会发生任何事情，但异步程序通常有许多这样的任务。

尽管异步编程可以加快应用程序速度，但它通常会导致程序更加复杂。程序员需要跟踪异步操作完成后恢复工作所需的所有状态。纵观古今，这是一项繁琐且容易出错的任务。

## 编译时绿色线程

Rust 使用名为`async/await`的功能来实现异步编程。执行异步操作的函数使用`async`标记。在我们的例子中，`connect`函数的定义大概长这样：

```rust
use mini_redis::Result;
use mini_redis::client::Client;
use tokio::net::ToSocketAddrs;

pub async fn connect<T: ToSocketAddrs>(addr: T) -> Result<Client> {
    // ...
}
```

`async fn`定义看起来像常规的同步函数，但是里面的操作是异步的。Rust 在**编译时**会把`async fn`转换为异步运行的例程(routine)。在`async fn`中任何调用`.await`时，都会把控制权返回给线程。当操作在后台进行时，该线程可以执行其他工作。

> **warning**
> 尽管其他语言也实现了 [`async/await`](https://en.wikipedia.org/wiki/Async/await)，但 Rust 中采用了独特的方法。主要是，Rust 中的异步操作都是**懒惰的(lazy)**。这会导致运行时语义与其他语言不同。

如果还是不太明白，别担心。我们将会在整个指南中探索更多`async/await`的内容。

## 使用`async/await`

异步函数可以像其他 Rust 函数一样被调用。然而，调用这些函数不会立刻执行函数体。相反的，调用`async fn`返回了一个代表操作的值。概念上类似于零参数闭包。要实际执行该操作，应该在返回值上使用`.await`操作符。

例如以下的程序：

```rust
async fn say_world() {
    println!("world");
}

#[tokio::main]
async fn main() {
    // 调用 `say_world()` 并没有执行 `say_world()` 的函数体.
    let op = say_world();

    // 这个 println! 会先执行
    println!("hello");

    // 在 `op` 上调用 `.await` 才会开始执行 `say_world`.
    op.await;
}
```

输出：

```
hello
world
```

`async fn`的返回值是实现了[Future](https://doc.rust-lang.org/std/future/trait.Future.html) trait 的匿名类型。

## 异步`main`函数

用于启动应用的主函数(main function)与其他大多数 Rust 包中的不同。

1. 是一个`async fn`异步函数
2. 带了`#[tokio::main]`注解

`async fn`可以让我们进入异步上下文。然而，异步函数必须由运行时来执行。这个运行时包含了异步任务的调度程序，提供事件 I/O，计时器等等。运行时并不会自动启动，所以需要 main 函数来启动它。

`#[tokio::main]`是一个宏。会把`async fn main()`转换为同步的`fn main()`，初始化运行时实例，并执行异步的 main 函数。

例如接下来：

```rust
#[tokio::main]
async fn main() {
    println!("hello");
}
```

可以转换为：

```rust
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    })
}
```

Tokio 运行时的详细信息将会在稍后介绍。

## Cargo features

当本教程依赖 Tokio 时, 将启用`full` feature(全部功能标志):

```toml
tokio = { version = "1", features = ["full"] }
```

Tokio 具有很多功能 (TCP, UDP, Unix sockets, timers, sync
utilities, multiple scheduler types, etc)。并非所有程序都需要这些功能。当尝试优化编译时间或最终程序的占用空间时，应用可以决定**仅**使用的 feature。
