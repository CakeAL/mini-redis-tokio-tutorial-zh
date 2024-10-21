# Bridging with sync code 异步与同步代码共存

使用 Tokio 的大多数例子中，我们使用`#[tokio::main]`注解标记 main 函数，并让整个项目是异步的。

但某些时候，你可能只需要执行一小部分异步代码。详细信息可以看：[`spawn_blocking`](https://docs.rs/tokio/1/tokio/task/fn.spawn_blocking.html)。

其他情况下，把应用程序构建为大多数是同步，具有小部分或逻辑上不同的异步部分可能会更容易一些。例如，一个 GUI 应用可能需要在 main 线程运行 GUI 代码，并在另外一个线程运行 Tokio 运行时。

本节将介绍你该如何把 async/await 隔离到你的项目中的一小部分。

# `#[tokio::main]`是什么东西

`#[tokio::main]`宏会用一个非异步的 main 函数来替换你的 main 函数，当这个函数启动了运行时，之后就可以调用你的代码。比如：

```rust
#[tokio::main]
async fn main() {
    println!("Hello world");
}
```

可以通过宏转换为：

```rust
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            println!("Hello world");
        })
}
```

为了在我们项目中使用 async/await，我们可以做类似的操作，在适当的情况下利用[`block_on`](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.block_on)方法，来进入异步上下文。

# mini-redis 的同步接口

本小节中，我们将会介绍如何通过存储`Runtime`对象并使用`block_on`方法来为 mini-redis 构建一个同步接口。在下面，我们会讨论一些替代方法，和何时使用这些方法。

我们将会包装的接口是一个异步的`Client`类型。它有以下几个方法，并且我们会实现这些方法的阻塞版本：

- [Client::get](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.get)
- [Client::set](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.set)
- [Client::set_expires](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.set_expires)
- [Client::publish](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.publish)
- [Client::subscribe](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.subscribe)

为此，我们创建一个新文件，叫`src/clients/blocking_client.rs`并通过包装异步`Client`类型的结构体来初始化。

```rust
use tokio::net::ToSocketAddrs;
use tokio::runtime::Runtime;

pub use crate::clients::client::Message;

/// 与 Redis server 建立连接。
pub struct BlockingClient {
    /// 异步的 `Client`.
    inner: crate::clients::Client,

    /// 一个 `current_thread` 运行时，用来在
    /// 一个阻塞的环境下对异步 client 执行操作
    rt: Runtime,
}

impl BlockingClient {
    pub fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<BlockingClient> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        // 通过运行时调用异步的 connect method
        let inner = rt.block_on(crate::clients::Client::connect(addr))?;

        Ok(BlockingClient { inner, rt })
    }
}
```

这里，我们把包含构造函数的 impl 作为我们第一个示例，来展示如何在非异步上下文中执行异步方法。我们通过在 Tokio 的[`Runtime`](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html)类型上使用[`block_on`](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.block_on)方法，这可以执行一个异步方法，并返回结果。

一个很重要的细节，我们使用了[`current_thread`](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_current_thread)运行时。通常当我们使用 Tokio 时，你可能使用默认的[`multi_thread`](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_multi_thread)运行时，当它运行时，会生成一堆后台线程，以便于它可以有效地同时运行很多事情。但在我们使用情况中，我们只一次做一件事，所以使用多线程没有任何好处。这让`current_thread`运行时非常适合，因为它不会生成任何线程。

[`enable_all`](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.enable_all)在 Tokio 运行时上调用了 IO 和定时器驱动程序。如果没启用，运行时就不会执行 IO 和定时器。

> **warning**  
> 因为`current_thread`运行时不会生成新线程，只会等待`block_on`调用。一旦`block_on`返回，这个运行时上所有生成的任务就会冻结，直到你再次调用`block_on`。如果生成的任务必须在没调用`block_on`时保持运行，使用`multi_threaded`运行时。

一旦我们有了这个结构体，大多数方法就很容易实现了：

```rust
use bytes::Bytes;
use std::time::Duration;

impl BlockingClient {
    pub fn get(&mut self, key: &str) -> crate::Result<Option<Bytes>> {
        self.rt.block_on(self.inner.get(key))
    }

    pub fn set(&mut self, key: &str, value: Bytes) -> crate::Result<()> {
        self.rt.block_on(self.inner.set(key, value))
    }

    pub fn set_expires(
        &mut self,
        key: &str,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        self.rt.block_on(self.inner.set_expires(key, value, expiration))
    }

    pub fn publish(&mut self, channel: &str, message: Bytes) -> crate::Result<u64> {
        self.rt.block_on(self.inner.publish(channel, message))
    }
}
```

`Client::subscribe`方法更有趣，因为它可以转换 Client 变成 Subscriber 对象。我们可以通过以下方式实现：

```rust
/// 已进入 发布/订阅 模式的客户端.
///
/// 一旦客户端订阅了一个频道，它就只能处理 发布/订阅
/// 相关的指令。`BlockingClient` 类型是用来转换
/// 为一个 `BlockingSubscriber` 类型，这样才能
/// 阻止调用 非发布/订阅 的指令。
pub struct BlockingSubscriber {
    /// 异步的 `Subscriber`.
    inner: crate::clients::Subscriber,

    /// 一个 `current_thread` 运行时，用来在
    /// 一个阻塞的环境下对异步 client 执行操作
    rt: Runtime,
}

impl BlockingClient {
    pub fn subscribe(self, channels: Vec<String>) -> crate::Result<BlockingSubscriber> {
        let subscriber = self.rt.block_on(self.inner.subscribe(channels))?;
        Ok(BlockingSubscriber {
            inner: subscriber,
            rt: self.rt,
        })
    }
}

impl BlockingSubscriber {
    pub fn get_subscribed(&self) -> &[String] {
        self.inner.get_subscribed()
    }

    pub fn next_message(&mut self) -> crate::Result<Option<Message>> {
        self.rt.block_on(self.inner.next_message())
    }

    pub fn subscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.subscribe(channels))
    }

    pub fn unsubscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.unsubscribe(channels))
    }
}
```

这样，`subscribe`方法就可以首先使用运行时转换异步`Client`到一个异步的`Subscriber`。然后，它会把`Subscribe`和运行时存储在一起，并使用`block_on`实现各种方法。

# 其他方法

上面小节解释了实现同步包装器的最简单方法，但不是唯一的方法。下面的方法有：

- 创建一个运行时，在异步代码上调用`block_on`。
- 创建一个运行时，在上面`spawn`任务。
- 在独立的线程中运行一个运行时，给它发送消息。

我们已经了解第一种方法了，剩余的两种在下面。

## 在一个运行时上生成任务

[运行时](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html)对象有一个方法，叫[`spawn`](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.spawn)。当你调用这个方法，你可以创建一个跑在这个运行时的新后台任务。例如：

```rust
use tokio::runtime::Builder;
use tokio::time::{sleep, Duration};

fn main() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(10);
    for i in 0..10 {
        handles.push(runtime.spawn(my_bg_task(i)));
    }

    // 做一些在后台任务执行时消耗时间的操作
    std::thread::sleep(Duration::from_millis(750));
    println!("Finished time-consuming task.");

    // 等待所有任务完成
    for handle in handles {
        // `spawn` 方法返回了 `JoinHandle`。`JoinHandle`是
        // 一个 future，所以我们可以使用 `block_on` 来等待。
        runtime.block_on(handle).unwrap();
    }
}

async fn my_bg_task(i: u64) {
    // 通过减法，i 值较大的任务会休眠更短的时间
    let millis = 1000 - 50 * i;
    println!("Task {} sleeping for {} ms.", i, millis);

    sleep(Duration::from_millis(millis)).await;

    println!("Task {} stopping.", i);
}
```

```
Task 0 sleeping for 1000 ms.
Task 1 sleeping for 950 ms.
Task 2 sleeping for 900 ms.
Task 3 sleeping for 850 ms.
Task 4 sleeping for 800 ms.
Task 5 sleeping for 750 ms.
Task 6 sleeping for 700 ms.
Task 7 sleeping for 650 ms.
Task 8 sleeping for 600 ms.
Task 9 sleeping for 550 ms.
Task 9 stopping.
Task 8 stopping.
Task 7 stopping.
Task 6 stopping.
Finished time-consuming task.
Task 5 stopping.
Task 4 stopping.
Task 3 stopping.
Task 2 stopping.
Task 1 stopping.
Task 0 stopping.
```

在上述示例中，我们在运行时上生成了 10 个后台任务，然后等待它们完成。例如，这可能是在图形应用程序中实现后台网络请求任务的好方法，因为网络请求很耗时，无法在主 GUI 线程上运行它们。所以，你可以在后台运行的 Tokio 运行时上生成请求，并当任务请求完成时，将消息发送回到 GUI 代码中，甚至如果你想实现一个进度条，可以让它们返回增量消息。

在这个例子中，将运行时配置为[`multi_thread`](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_multi_thread)非常重要。如果你更改为`current_thread`运行时，你就会发现耗时的任务会在任何后台任务开始前完成了。这是因为后台任务在`current_thread`运行时上生成，只有当在运行时上调用`block_on`期间才会执行，否则运行时没有其他任何地方运行它们。

这个例子通过在`spawn`生成的`JoinHandle`上调用`block_on`来等待生成的任务完成，但这不是唯一的方法，下面有一些替代方案：

- 使用消息传递管道，例如`tokio::sync::mpsc`
- 更改一个共享的值，例如一个`Mutex`。这是一个好方法，对于一个 GUI 中的进度条来说，因为 GUI 需要每一帧都读取共享值。

`spawn`方法也在`Handle`类型上可用。`Handle`类型可以被克隆来拿到很多运行时的 handle，并且每一个`Handle`都可以用于在运行时上生成新任务。

## 消息传递

第三种方法是生成一个运行时，并使用消息传递与其通信。对比前两种方法，它是最灵活的，你可以在下面看到一个最基本的示例：

```rust
use tokio::runtime::Builder;
use tokio::sync::mpsc;

pub struct Task {
    name: String,
    // 描述任务的信息
}

async fn handle_task(task: Task) {
    println!("Got task {}", task.name);
}

#[derive(Clone)]
pub struct TaskSpawner {
    spawn: mpsc::Sender<Task>,
}

impl TaskSpawner {
    pub fn new() -> TaskSpawner {
        // 建立通信管道。
        let (send, mut recv) = mpsc::channel(16);

        // 为新线程建立运行时
        //
        // 在生成新线程之后就创建运行时，这样可以更清晰的追踪error
        // 如果 `unwrap()` panic了。
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some(task) = recv.recv().await {
                    tokio::spawn(handle_task(task));
                }

                // 一旦所有的sender都已经走出作用域
                // `.recv()` 返回 None 并从循环中退出
                // 之后关闭线程
            });
        });

        TaskSpawner {
            spawn: send,
        }
    }

    pub fn spawn_task(&self, task: Task) {
        match self.spawn.blocking_send(task) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}
```

该示例可以通过多种方式配置。例如，你可以使用[`Semaphore`](https://docs.rs/tokio/1/tokio/sync/struct.Semaphore.html) (信号量)来限制处于活动状态的任务，或者你可以使用相反方向的管道来发送回一个响应对生成器(spawner)这儿。当你用这种方法生成一个运行时时，这是一个[actor](https://ryhl.io/blog/actors-with-tokio/)类型。
