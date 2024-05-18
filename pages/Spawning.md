# Spawning 生成任务

现在我们来实现 Redis 服务端。

首先，把上一节客户端`SET/GET`代码移动到 example 文件中。这样我们就可以在服务器上运行它。

```bash
$ mkdir -p examples
$ mv src/main.rs examples/hello-redis.rs
```

然后新建一个空文件`src/main.rs`，并继续。

# 接收套接字

我们 Redis 服务器要做的第一件事就是接收 TCP 套接字。这是通过绑定[`tokio::net::TcpListener`](https://docs.rs/tokio/1/tokio/net/struct.TcpListener.html)到**6379**端口来完成的。

> **info**
> Tokio 很多类型名与 Rust 标准库中同步类型相同。在合理情况(make sense)下，Tokio 暴露了与 std 相同的 API，但是使用`async fn`。

然后在一个 loop 循环中接收套接字。每个套接字都会被处理然后关闭。现在，我们将会读取指令，将其打印到 stdout 并响应一个 error。

`src/main.rs`

```rust
use tokio::net::{TcpListener, TcpStream};
use mini_redis::{Connection, Frame};

#[tokio::main]
async fn main() {
    // 把listener绑定到这个地址
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        // 第二个值包含了新连接的IP和端口
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(socket: TcpStream) {
    // 这个 `Connection` 让我们读/写Redis **帧(frames)** 而不是
    // 比特流. 这个 `Connection` 类型是由 mini-redis 定义的。
    let mut connection = Connection::new(socket);

    if let Some(frame) = connection.read_frame().await.unwrap() {
        println!("GOT: {:?}", frame);

        // 响应一个error
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await.unwrap();
    }
}
```

现在，运行这个接收循环：

```bash
$ cargo run
```

在独立的终端窗口，运行`hello-redis`例子（之前章节写的`SET/GET`命令）：

```bash
$ cargo run --example hello-redis
```

输出应该是：

```
Error: "unimplemented"
```

服务端的终端窗口，输出应该是：

```
GOT: Array([Bulk(b"set"), Bulk(b"hello"), Bulk(b"world")])
```

# 并发

我们的服务端有个小问题（除了仅仅响应 error 之外）。它一次只能处理一个入站请求。当一个连接被接收，服务端将会停留在接收循环里，直到响应完全写入套接字。

我们希望 Redis 服务端可以处理许多并发请求。所以我们需要添加一些并发功能。

> **info**
> 并发和并行不是一回事。如果你在两个任务之间交替，那你就是并发地处理两个任务，而不是并行的。如果是并行的话，你需要两个人，每个人专门负责一个任务。 \
> 使用 Tokio 的一个优点就是，异步代码允许您同时处理多个任务，而不用使用普通线程并行得处理它们。事实上，Tokio 可以在单个线程上运行多个任务！

为了同时处理连接，将会为每个入站连接生成一个新任务。连接会在此任务上进行处理。

接收循环变成下面这样：

```rust
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // 每个传入的套接字都会被生成一个新任务。套接字（的所有权）
        // 被移动到新任务并且在那儿处理。
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}
```

## 任务

一个 Tokio 任务就是一个异步绿色线程。它们是通过`async`块传递给`tokio::spawn`来创建的。`tokio::spawn`函数返回一个`JoinHandle`，调用者可以使用它来与生成的任务进行交互。`async`块可以有返回值。调用者可以使用`.await`作用于`JoinHandle`上，来获取返回值。

例如：

```rust
#[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        // 做一些异步任务
        "return value"
    });

    // 做一些其他任务

    let out = handle.await.unwrap();
    println!("GOT {}", out);
}
```

等待`JoinHandle`返回一个`Result`。当一个任务执行中发生错误，`JoinHandle`会返回一个`Err`。任务 Panic 或者因为运行时关闭而被强制取消时，会发生这种情况。

任务是调度器管理的执行单元。生成任务会提交给 Tokio 调度器，然后调度器会确保这个任务在有工作时执行。生成的任务可以在生成它的同一个线程上执行，也可以在不同的线程上执行。任务生成后也可以在线程间移动。

Tokio 中的任务非常轻量。在底层，它们只需要一次性分配 64 字节内存。应用应该可以直接生成数千计，甚至数百万的任务。

## `'static` bound

当在 Tokio 运行时生成一个任务时，它的类型生命周期必须是`'static`的。这意味着生成的任务不能包含任何任务外的数据的引用。

> **info**
> 普遍认为`'static`意味着“永远存在”，但事实并非如此。仅仅因为一个`'static`值不意味着存在内存泄漏。你可以在[Rust 生命周期误会](https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program)中阅读更多。

例如，下面的代码不能通过编译：

```rust
use tokio::task;

#[tokio::main]
async fn main() {
    let v = vec![1, 2, 3];

    task::spawn(async {
        println!("Here's a vec: {:?}", v);
    });
}
```

尝试编译会导致以下错误：

```
error[E0373]: async block may outlive the current function, but
              it borrows `v`, which is owned by the current function
 --> src/main.rs:7:23
  |
7 |       task::spawn(async {
  |  _______________________^
8 | |         println!("Here's a vec: {:?}", v);
  | |                                        - `v` is borrowed here
9 | |     });
  | |_____^ may outlive borrowed value `v`
  |
note: function requires argument type to outlive `'static`
 --> src/main.rs:7:17
  |
7 |       task::spawn(async {
  |  _________________^
8 | |         println!("Here's a vector: {:?}", v);
9 | |     });
  | |_____^
help: to force the async block to take ownership of `v` (and any other
      referenced variables), use the `move` keyword
  |
7 |     task::spawn(async move {
8 |         println!("Here's a vec: {:?}", v);
9 |     });
  |
```

这种情况是因为在默认情况下，变量不会`移动move`到异步块中。`v` vector 仍被`main`函数所拥有。`println!`行借用了`v`。Rust 编译器向我们解释了这一点，甚至给出了如何修改的建议！将第 7 行更改为`task::spawn(async move {`将会让编译器把` v``移动move `到生成的任务中。现在，这个任务拥有了它自己的数据，使其成为`'static`的。

如果必须同时从多个任务访问单个数据，那就必须使用`Arc`等同步原语来共享。

注意，刚刚的错误信息也提到了参数类型比`'static`生命周期*活得更长*。这种术语可能相当令人困惑，因为`'static`生命周期会持续到程序结束，所以如果比它活得还长，是不是出现了内存泄漏？解释是，它是个*类型*，不是必须比`'static`活得更长的*值*，并且这个值有可能在类型不再有效之前被摧毁。

当我们说一个值是`'static`时，意思是永远保持该值不是错误的。这很重要，因为编译器无能推断新生成的任务会持续多长时间。我们必须确保任务能够永远活着，这样 Tokio 就可以让任务运行它需要长的时间。

Info 框链接的文章使用了术语“受`'static`限制”而不是“它的类型活得比`'static`长”或者“值为`'static`，引用于`T: 'static`”。之前这些都是一个意思，但是与`&'static T`中的`'static`注解不同（这是个引用的生命周期，前面的不是）。

## `Send` bound

由`tokio::spawn`生成的任务**必须**实现了`Send`。这使得Tokio运行时可以在线程之间移动任务，同时在`.await`处挂起任务。

当**所有**通过**调用**`.await`的数据都是`Send`的，任务才能是`Send`的。这有点微妙。当`.await`被调用，任务返回给调度器。下一次任务被执行时，它将会从上次返回时恢复。为了实现这种功能，任务必须保存`.await`**之后的**所有状态。如果这个状态是`Send`的，即可以跨线程移动，那么任务本身就可以跨线程移动。相反的，如果状态没实现`Send`，那任务也不是`Send`的。

例如，这个可以运行：

```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        // 这歌代码块强制 `rc` 在 `.await` 之前 drop 了
        {
            let rc = Rc::new("hello");
            println!("{}", rc);
        }

        // `rc` 没再用。 当任务返回给(yield)调度器，它**没再**持续
        yield_now().await;
    });
}
```

这个不行：

```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        let rc = Rc::new("hello");

        // `rc` 在 `.await` 之后使用了，它必须在任务状态中持续
        yield_now().await;

        println!("{}", rc);
    });
}
```

尝试编译这段代码会导致：

```
error: future cannot be sent between threads safely
   --> src/main.rs:6:5
    |
6   |     tokio::spawn(async {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    | 
   ::: [..]spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in
    |                          `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait
    |       `std::marker::Send` is not  implemented for
    |       `std::rc::Rc<&str>`
note: future is not `Send` as this value is used across an await
   --> src/main.rs:10:9
    |
7   |         let rc = Rc::new("hello");
    |             -- has type `std::rc::Rc<&str>` which is not `Send`
...
10  |         yield_now().await;
    |         ^^^^^^^^^^^^^^^^^ await occurs here, with `rc` maybe
    |                           used later
11  |         println!("{}", rc);
12  |     });
    |     - `rc` is later dropped here
```

我们将会在[下一章节](https://tokio.rs/tokio/tutorial/shared-state#holding-a-mutexguard-across-an-await)更深入探讨此类特殊情况。

# 存储值

我们现在将会实现`process`函数来处理传入的指令。我们将会用一个`HashMap`来存储值。`SET`指令会插入到`HashMap`中，`GET`会加载它们。此外，我们会使用循环来为每个连接接收多条指令。

```rust
use tokio::net::TcpStream;
use mini_redis::{Connection, Frame};

async fn process(socket: TcpStream) {
    use mini_redis::Command::{self, Get, Set};
    use std::collections::HashMap;

    // HashMap用来存储值
    let mut db = HashMap::new();

    // 由 `mini-redis` 提供的 Connection ，处理解析从套接字传过来的帧
    // (handles parsing frames from the socket)
    let mut connection = Connection::new(socket);

    // 使用 `read_frame` 来从连接中接收一条指令。
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                // 值被存储为 `Vec<u8>`
                db.insert(cmd.key().to_string(), cmd.value().to_vec());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    // `Frame::Bulk` 期望数据类型是 `Bytes`。
                    // 这种类型在本教程中稍后介绍。现在，
                    // `&Vec<u8>` 可以使用 `into()` 转换为 `Bytes`。
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // 写回响应，传回给客户端
        connection.write_frame(&response).await.unwrap();
    }
}
```

现在，启动服务端：

```bash
$ cargo run
```

然后在另外的终端窗口，运行`hello-redis`例子：

```bash
$ cargo run --example hello-redis
```

现在，输出应该是：

```
got value from the server; result=Some(b"world")
```

现在，我们可以获取和设置值，但还有个问题：连接之间，值不能被共享。如果另一个套接字连接，并尝试`GET``hello`key，它将找不到任何内容。

你可以在[这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/spawning/src/main.rs)找到完整代码。

下一节，我们将会为所有套接字实现持久数据。