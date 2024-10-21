# Channels 管道

现在我们已经了解了一些关于 Tokio 并发的知识，让我们把它应用到客户端。把我们之前写的服务端代码放到独立的二进制文件中。

```bash
$ mkdir src/bin
$ mv src/main.rs src/bin/server.rs
```

并且创建一个新的二进制文件来存放客户端代码：

```bash
touch src/bin/client.rs
```

在这个文件中，我们将会编写本页的代码。每当想要运行它时，你需要先在一个独立的终端窗口启动服务器：

```bash
$ cargo run --bin server
```

然后运行客户端，也是在独立的终端窗口：

```bash
$ cargo run --bin client
```

完成之后，让我们开始 coding！

假设我们要并发地运行两个 Redis 指令。我们可以为每个指令生成一个任务。然后这两个命令将并发处理。

首先，我们可能这样写：

```rust
use mini_redis::client;

#[tokio::main]
async fn main() {
    // 与服务器建立连接
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // 产生两个任务，一个get一个key，另一个set一个key
    let t1 = tokio::spawn(async {
        let res = client.get("foo").await;
    });

    let t2 = tokio::spawn(async {
        client.set("foo", "bar".into()).await;
    });

    t1.await.unwrap();
    t2.await.unwrap();
}
```

这个不会编译成功，因为两个任务可能在随机时刻访问`client`连接。但是`Client`并没有实现`Copy`，因此如果没有一些代码来让它共享，就不能编译成功。此外，`Client::set`参数类型是`&mut self`的，这意味着调用它需要独占访问权限。我们可以为每个任务单独开一个连接，但是这样并不理想。我们不能用`std::sync::Mutex`，因为调用`.await`时需要持有锁。我们可以用`tokio::sync::Mutex`，但是这只能让我们处理单个请求。如果客户端实现了[流水线](https://redis.io/topics/pipelining)，异步锁会导致连接的利用率不足。

## 消息传递 Message passing

解决办法是用消息传递。该模式可以生成一个专用任务来处理`client`资源。希望发出请求的任务都可以发送一个消息到`client`任务。然后`client`任务代表这些发送者发出请求，并把响应返回来给发送者。

用这种方式，建立单个连接。管理`client`的任务能够获得独占的访问权限，以便于调用`get`和`set`。此外，这个管道还可以充当缓冲区。在`client`任务忙时，也可能有操作被发送到`client`任务。一旦`client`任务空闲，可以处理新的请求，它就会从管道中接收下一个请求。这可以带来更好的吞吐量，并且可以扩展支持连接池。

# Tokio管道原语

Tokio提供了[许多管道](https://docs.rs/tokio/1/tokio/sync/index.html)，每一种都有不同的用处。

- [mpsc](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html)：多个生产者，单个消费者管道。可以向管道发送很多值。
- [oneshot](https://docs.rs/tokio/1/tokio/sync/oneshot/index.html)：单个生产者，单个消费者管道。一次只能发送一个值。
- [broadcast](https://docs.rs/tokio/1/tokio/sync/broadcast/index.html)：多个生产者，多个消费者管道。可以向管道发送很多值，每个接收者都能看到每个被发送到值。
- [watch](https://docs.rs/tokio/1/tokio/sync/watch/index.html)：多个生产者，多个消费者管道。可以向管道发送很多值，但是没有历史记录。接收者只能看到最新的值。

如果你需要一个多生产者多消费者管道，其中只能有一个消费者看到每一条消息，你可以使用[`async-channel`](https://docs.rs/async-channel/)库。还有一些在异步Rust以外用的库，比如[`std::sync::mpsc`](https://doc.rust-lang.org/stable/std/sync/mpsc/index.html)和[`crossbeam::channel`](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html)。这些管道通过阻塞当前线程来等待消息，这在异步代码中是不允许的。

在本节课中，我们将会使用[mpsc](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html)和[oneshot](https://docs.rs/tokio/1/tokio/sync/oneshot/index.html)。其他类型的消息传递通道将会在后续章节中探讨。本节的完整代码可以在[这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/channels/src/main.rs)找到。

# 定义消息类型

大多数情况下，当使用消息传递时，接收消息的任务会响应多个指令。在我们的例子中，此任务会对`GET`和`SET`指令做出响应。对此建立模型，我们首先定义一个`Command`枚举，并包含每个指令类型的变体。

```rust
use bytes::Bytes;

#[derive(Debug)]
enum Command {
    Get {
        key: String,
    },
    Set {
        key: String,
        val: Bytes,
    }
}
```

# 创建管道

在`main`函数中，创建一个`mpsc`管道。

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // 创建一个容量为32的新管道
    let (tx, mut rx) = mpsc::channel(32);

    // ... 其他代码一会儿在这儿写
}
```

`mpsc`管道是用来**发送(send)**指令到管理redis连接的任务的。多生产者能力可以允许消息从很多任务发送过来。创建通道会返回两个值，一个发送者(sender)和一个接收者(receiver)。这两个句柄(handle)可以分开使用。它们可以被移动(moved)到不同的任务中。

创建的管道容量是32。如果发送消息的速度比接收快，管道就会存储它们。一旦32条消息都被存储到管道里，调用`send(...).await`就会进入休眠状态(go to sleep)，直到接收者移除了(处理了)一条消息。

从多个任务发送是由**克隆**`发送者`来完成的。例如：

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel(32);
    let tx2 = tx.clone();

    tokio::spawn(async move {
        tx.send("sending from first handle").await.unwrap();
    });

    tokio::spawn(async move {
        tx2.send("sending from second handle").await.unwrap();
    });

    while let Some(message) = rx.recv().await {
        println!("GOT = {}", message);
    }
}
```

两条消息都被发送到单个接收者句柄。无法克隆`mpsc`管道的接收者。

当每个`发送者`都超出生命周期或其他原因被drop了，就不再能向管道中发送更多消息了。此时，`接收者`调用`recv`将返回`None`，这意味着所有的发送者都消失了，管道关闭了。

在我们的管理Redis连接任务的例子中，它知道一旦管道关闭了，Redis连接就可以关了，因为这个连接再也用不到了。

# 生成管理任务

接下来，生成一个任务处理来自管道中的消息。首先，客户端与Redis建立连接。然后，接收到的指令经由Redis连接发出。

```rust
use mini_redis::client;
// `move` 关键字用来 **移动** `rx` 的所有权到任务中。
let manager = tokio::spawn(async move {
    // 与服务端建立连接
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // 开始接收消息
    while let Some(cmd) = rx.recv().await {
        use Command::*;

        match cmd {
            Get { key } => {
                client.get(&key).await;
            }
            Set { key, val } => {
                client.set(&key, val).await;
            }
        }
    }
});
```

现在，更新两个任务的代码来通过管道发送指令，而不是直接通过Redis连接发送。

```rust
// `Sender` 句柄被移动到任务里. 因为这儿有俩任务，
// 我们需要另一个 `Sender`
let tx2 = tx.clone();

// 生成俩任务，一个get一个key，一个set一个key
let t1 = tokio::spawn(async move {
    let cmd = Command::Get {
        key: "foo".to_string(),
    };

    tx.send(cmd).await.unwrap();
});

let t2 = tokio::spawn(async move {
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: "bar".into(),
    };

    tx2.send(cmd).await.unwrap();
});
```

在`main`函数底部，我们调用`.await`来等待连接句柄(join handles)，以确保在进程退出之前全部完成这些指令。

```rust
t1.await.unwrap();
t2.await.unwrap();
manager.await.unwrap();
```

# 接收响应

最后一步，是接收从管理任务中返回的响应。`GET`指令需要获取到值，`SET`指令需要知道这个操作是否成功完成。

为了传递响应，使用`oneshot`管道。`oneshot`管道是单个生产者，单个消费者管道，针对发送单个值进行了优化。在我们的例子中，单个值就是响应。

与`mpsc`类似，`oneshot::channel()`返回一个发送者和一个接收者句柄。

```rust
use tokio::sync::oneshot;

let (tx, rx) = oneshot::channel();
```

与`mpsc`不同的地方在于没有指定容量，因为容量就是一。此外，两个句柄都不能克隆。

为了接收从管理任务传过来的响应，在发送指令之前，需要建立一个`oneshot`管道。管道中的`发送者`这块被包含在了管理任务的指令中。接收者那块用来接收响应。

首先，更新`Command`枚举来包含`发送者`。为了方便，给`发送者`起个别名。

```rust
use tokio::sync::oneshot;
use bytes::Bytes;

/// 多个不同的命令在单个管道上复用
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}

/// 由请求者提供，并由管理任务来发送指令的响应，返回给请求者
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;
```

现在，更新发出命令的任务代码，来包含`oneshot::Sender`。

```rust
let t1 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Get {
        key: "foo".to_string(),
        resp: resp_tx,
    };

    // 发送GET请求
    tx.send(cmd).await.unwrap();

    // 等待响应
    let res = resp_rx.await;
    println!("GOT = {:?}", res);
});

let t2 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: "bar".into(),
        resp: resp_tx,
    };

    // 发送SET请求
    tx2.send(cmd).await.unwrap();

    // 等待响应
    let res = resp_rx.await;
    println!("GOT = {:?}", res);
});
```

最后，更新管理任务，来通过`oneshot`管道发送响应。

```rust
while let Some(cmd) = rx.recv().await {
    match cmd {
        Command::Get { key, resp } => {
            let res = client.get(&key).await;
            // 使用 `_` 忽略错误
            let _ = resp.send(res);
        }
        Command::Set { key, val, resp } => {
            let res = client.set(&key, val).await;
            // 使用 `_` 忽略错误
            let _ = resp.send(res);
        }
    }
}
```

在`onsshot::Sender`上调用`send`会立即完成，并且不再需要`.await`。这是这是因为`oneshot`管道的`send`方法会总是会立即失败或成功，不需要等待。
当接收者那部分被drop了，在oneshot通道上发送值就会返回`Err`。这意味着接收者不再对响应作出反应。在我们的场景中，接收者对接收事件不再响应是合理的。`resp.send()`返回的`Err`不需要处理。

你可以在[这儿](https://github.com/tokio-rs/website/blob/master/tutorial-code/channels/src/main.rs)找到完整代码。

# 对消息通道进行限制

每当引入并发或排队时，确保排队是有界的并且系统能够负载得起的非常重要。无界队列最终将会填满所有可用内容，并导致系统以不可预测的方式崩掉。

Tokyo会关注避免隐式排队。很大异步原因就是因为异步操作是惰性的(lazy)。考虑以下情况：

```rust
loop {
    async_op();
}
```
如果异步操作都马上开始运行，那么这个循环会重复排队一个新的`async_op`任务，而不会确保之前的任务操作完成。这会导致隐式无界排队。基于回调的系统和基于**eager** future的系统很容易受到这种情况影响。

然而，Tokio和异步Rust**不会**让上述代码段中`async_op`运行。这是因为`.await`没有被调用。如果代码块使用了`.await`，那么循环在重新开始时会等待上一次操作完成。

```rust
loop {
    // 在 `async_op` 完成之前不会重复
    async_op().await;
}
```

必须明确地引入并发和队列。执行此操作的方法有：

- `tokio::spawn`
- `select!`
- `join!`
- `mpsc::channel`

这样做时，注意确保总的并发量是有限的。例如，当写TCP接收循环时，确保总的打开的套接字(socket)是有限的。当使用`mpsc::channel`管道，选择管道的容量。具体的限定值取决于是什么应用。

小心地选择良好的边界是编写可靠Tokio应用的重要组成部分。