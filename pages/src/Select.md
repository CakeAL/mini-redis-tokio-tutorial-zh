# Select 选择先完成的

现在，当我们已经想要向系统添加并发时，我们可以生成新任务。现在我们将介绍一些Tokio并发执行异步代码的其他方法。

# `tokio::select!`

`tokio::select!`宏允许在多个异步计算等待，并当**单个**计算完成时返回。

例如：

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```
这里使用了两个oneshot管道。任一管道都可以首先完成。`select!`语句会在两个管道上等待，并将`val`绑定到任务返回到值。当`tx1`或`tx2`完成时，这个语句块将被执行。

**未完成的**分支将会被drop。在上面例子，计算过程等待每个管道的`oneshot::Receiver`。尚未完成的管道的`oneshot::Receiver`将会被drop。

## 消除 Cancellation

对于异步Rust来说，消除任务是通过drop future来进行的。回想一下[“深入异步”](https://tokio.rs/tokio/tutorial/async)，异步Rust操作是通过使用future实现的，而future是惰性的(lazy)。该操作仅在future被轮询的时候实现。如果future被drop，操作就无法继续了，因为有关状态已经被drop了。

这表明，有时异步操作会在生成后台任务或在后台运行其他操作。例如，在上面的示例中，生成了一个任务来发送一个消息，然后返回。通常，后台任务将执行一些计算来生成值。

Future或其他类型都可以实现`Drop`来清理后台资源。Tokio的`oneshot::Receiver`实现了`Drop`，通过向`Sender`部分发送一个关闭通知。sender部分就可以接收到这个通知，然后丢弃正在进行中的操作来drop。

```rust
use tokio::sync::oneshot;

async fn some_operation() -> String {
    // 在这里计算一些值
}

#[tokio::main]
async fn main() {
    let (mut tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        // 在某些操作和 oneshot 的 `closed` 通知上选择先完成的
        tokio::select! {
            val = some_operation() => {
                let _ = tx1.send(val);
            }
            _ = tx1.closed() => {
                // `some_operation()` 被取消了
                // 任务完成并且 `tx1` 被drop
            }
        }
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

## `Future`实现

为了更好理解`select!`是如何工作的，让我们看看假设的`Future`实现什么样。这是一个简化版本。实际上，`select!`包含一些其他功能，例如随机选择首先轮询的分支。

```rust
use tokio::sync::oneshot;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct MySelect {
    rx1: oneshot::Receiver<&'static str>,
    rx2: oneshot::Receiver<&'static str>,
}

impl Future for MySelect {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if let Poll::Ready(val) = Pin::new(&mut self.rx1).poll(cx) {
            println!("rx1 completed first with {:?}", val);
            return Poll::Ready(());
        }

        if let Poll::Ready(val) = Pin::new(&mut self.rx2).poll(cx) {
            println!("rx2 completed first with {:?}", val);
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    // 使用 tx1 和 tx2

    MySelect {
        rx1,
        rx2,
    }.await;
}
```

`MySelect`future包含了每个分支的future。当`MySelect`被轮询时，第一个分支将被轮询。如果它就绪了，使用该值完成`MySelect`。`.await`收到future的输出之后，future被drop。这会让两个分支的future都drop。由于另外一个分支没完成，该操作实际上被消除了。

请回想一下上一节内容：

> **info**  
> 当一个future返回了`Poll::Pending`，它**必须**确保在将来某时给waker发出信号。忘了这样做会导致任务被无限期挂起。

`MySelect`实现中没有显式的使用`Context`参数。相反，waker要求是传递`cx`到内部的future。由于内部的future也得满足waker的需求，当从内部的future接收到`Poll::Pending`时，外部只返回`Poll::Pending`，这样`MySelect`也能满足waker的需求。


# 语法

`select!` 宏可以处理两个以上分支。当前的限制是64个分支。每个分支的结构如下：

```
<pattern> = <async expression> => <handler>,
<模式> = <异步表达式> => <处理句柄>,
```
当`select!`宏计算时，所有的`<async expression>`都会被并发执行。当某个异步表达式完成时，结果将会与`<pattern>`进行模式匹配。如果结果与模式匹配，则drop其他所有异步表达式，并执行`<handler>`。`<handler>`表达式可以访问由`<pattern>`建立的任何绑定。

`<pattern>`的基本例子是一个变量名，异步表达式的结果绑定到变量名，然后`<handler>`可以访问这个变量。这就是为什么最开始的例子中，`<pattern>`是`val`，并`<handler>`可以访问`val`。

如果`<pattern>`**不**匹配异步计算的结果，那么剩余的异步表达式将继续并发执行，直到有一个完成。此时，会对结果执行相同的逻辑。

因为`select!`接收任何异步表达式，所以定义更复杂的异步计算是可能的。

在这里，我们选择`oneshot`管道和一个TCP连接的输出，谁先输出。


```rust
use tokio::net::TcpStream;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx, rx) = oneshot::channel();

    // 生成一个任务向 oneshot 管道发送一个消息
    tokio::spawn(async move {
        tx.send("done").unwrap();
    });

    tokio::select! {
        socket = TcpStream::connect("localhost:3465") => {
            println!("Socket connected {:?}", socket);
        }
        msg = rx => {
            println!("received message first {:?}", msg);
        }
    }
}
```

在这里，我们选择一个oneshot和从一个`TcpListener`接收套接字，谁先完成。

```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        tx.send(()).unwrap();
    });

    let mut listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        _ = async {
            loop {
                let (socket, _) = listener.accept().await?;
                tokio::spawn(async move { process(socket) });
            }

            // 帮帮 Rust 的类型推断
            Ok::<_, io::Error>(())
        } => {}
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

接收循环将会一直运行，直到遇到错误或`rx`接收到一个值。`_`模式表示我们忽略了异步计算的返回值。

# 返回值

`tokio::select!`宏返回`<handler>`表达式的结果。

```rust
async fn computation1() -> String {
    // .. 一些计算
}

async fn computation2() -> String {
    // .. 一些计算
}

#[tokio::main]
async fn main() {
    let out = tokio::select! {
        res1 = computation1() => res1,
        res2 = computation2() => res2,
    };

    println!("Got = {}", out);
}
```

正因如此，它要求**每个**分支的`<handler>`表达式均为相同类型。如果一个`select!`的表达式的输出是不需要的，最后把handler表达式的输出设为`()`。

# 错误处理

使用`?`运算符来传播表达式中的错误。这是否能用取决于是否`?`被用在异步表达式或handler中。在异步表达式中使用`?`会将错误传播到异步表达式之外。这会让异步表达式输出一个`Result`。在handler中使用`?`将会立即传播错误到`select!`表达式外部。让我们再看一下接收循环例子：

```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    // [初始化 `rx` oneshot 管道]

    let listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        res = async {
            loop {
                let (socket, _) = listener.accept().await?;
                tokio::spawn(async move { process(socket) });
            }

            // 帮帮 Rust 的类型推断
            Ok::<_, io::Error>(())
        } => {
            res?;
        }
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

注意`listener.accept().await?`。`?`运算符传播的错误绑定到了`res`。发生错误时，`res`会变成`Err(_)`。然后，在handler中再用`?`。`res?`语句会把错误传播到`main`之外。

# 模式匹配

回想一下`select!`宏分支语法是这样定义的：

```
<pattern> = <async expression> => <handler>,
```

到目前为止，我们都只对`<pattern>`绑定一个变量。其实，任何Rust模式都可以用。例如，假设我们从多个MPSC管道中接收数据，我们可能这样做：

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (mut tx1, mut rx1) = mpsc::channel(128);
    let (mut tx2, mut rx2) = mpsc::channel(128);

    tokio::spawn(async move {
        // 给 `tx1` and `tx2` 传点消息
    });

    tokio::select! {
        Some(v) = rx1.recv() => {
            println!("Got {:?} from rx1", v);
        }
        Some(v) = rx2.recv() => {
            println!("Got {:?} from rx2", v);
        }
        else => {
            println!("Both channels closed");
        }
    }
}
```

这个例子中，`select!`表达式会等待从`rx1`和`rx2`接收值。如果管道关闭，`recv()`会返回`None`。这**不会**匹配这个模式，该分支会被禁用。`select!`表达式会继续等待其他剩余的分支。

注意这个`select!`表达式包含一个`else`分支。这表示`select!`表达式必须计算出一个值。当使用模式匹配时，可能**没有任何一个**分支可以与其匹配。如果发生这种情况，就会计算`else`分支。

# 借用

当生成任务时，生成的异步表达式必须拥有其数据的所有权。`select!`宏则没有该限制。每个分支的异步表达式可以借用数据并并发操作。为了遵循Rust的借用规则，多个异步表达式可以不可变借用一条数据，**或者**一个异步表达式可以可变借用一条数据。

让我看一些例子。这里，我们将相同的数据发送到两个不同的TCP目的地。

```rust
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use std::io;
use std::net::SocketAddr;

async fn race(
    data: &[u8],
    addr1: SocketAddr,
    addr2: SocketAddr
) -> io::Result<()> {
    tokio::select! {
        Ok(_) = async {
            let mut socket = TcpStream::connect(addr1).await?;
            socket.write_all(data).await?;
            Ok::<_, io::Error>(())
        } => {}
        Ok(_) = async {
            let mut socket = TcpStream::connect(addr2).await?;
            socket.write_all(data).await?;
            Ok::<_, io::Error>(())
        } => {}
        else => {}
    };

    Ok(())
}
```

`data`变量在两个异步表达式都被不可变借用。当其中一个操作成功完成时，另外一个被drop。因为模式匹配了`Ok(_)`，如果一个表达式失败，另外一个会继续执行。

当涉及到每个分支的`<handler>`，`select!`会保证只有一个`<handler>`在运行。正因如此，每个`<handler>`都可以可变借用相同的数据（因为同时刻只有一个在运行）。

例如，这两个handler更改了`out`：

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    let mut out = String::new();

    tokio::spawn(async move {
        // 给 `tx1` and `tx2` 发送点数据。
    });

    tokio::select! {
        _ = rx1 => {
            out.push_str("rx1 completed");
        }
        _ = rx2 => {
            out.push_str("rx2 completed");
        }
    }

    println!("{}", out);
}
```

# 循环

`select!`宏总是在循环中启用。本小节将通过一些实例介绍在循环中使用`select!`宏的常见方法。让我们通过选择多个管道谁先完成开始：

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx1, mut rx1) = mpsc::channel(128);
    let (tx2, mut rx2) = mpsc::channel(128);
    let (tx3, mut rx3) = mpsc::channel(128);

    loop {
        let msg = tokio::select! {
            Some(msg) = rx1.recv() => msg,
            Some(msg) = rx2.recv() => msg,
            Some(msg) = rx3.recv() => msg,
            else => { break }
        };

        println!("Got {:?}", msg);
    }

    println!("All channels have been closed.");
}
```

这个示例选择了三个管道的接收者。当有其中一个管道接收到消息，消息会被写入STDOUT。当某个管道关闭时，`recv()`会返回None。通过使用模式匹配，`select!`宏会继续等待等待剩余的管道完成。当所有管道关闭，`else`分支会开始计算，然后循环终止。

`select!`宏会随机选择分支，来先检查是否就绪。当多个管道都有待处理值时，将随机选择一个管道接收。这是为了处理当接收循环处理消息比管道接收消息速度慢的情况。这意味着管道开始被填满。如果`select!`**不去**随机选一个分支来检查，每次的循环迭代中，`rx1`总是被首先检查。如果`rx1`总是有新消息，其他的管道将永远不会被检查。

> **info**  
> 如果`select!`计算时，多个管道有待处理的消息，只有一个管道的消息会被弹出。其他的管道都会保持不变，并且消息保留在管道中知道下一次循环迭代。没有消息会丢失。

## 恢复异步操作 Resuming an async operation


现在我们将展示如何跨多次调用`select!`来执行异步操作。在这个例子中，有一个类型为`i32`的MPSC管道，和一个异步函数。我们希望运行该异步函数，直到它从管道中接收到一个偶数。

```rust
async fn action() {
    // 一些异步逻辑
}

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(128);    
    
    let operation = action();
    tokio::pin!(operation);
    
    loop {
        tokio::select! {
            _ = &mut operation => break,
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    break;
                }
            }
        }
    }
}
```

注意，没在`select!`宏中调用`action()`，而是在循环**外**调用。`action()`的返回值给了`operation`，而且**没**调用`.await`。然后我们在`operation`上调用了`tokio::pin!`。

在`select!`循环体中，没传入`operation`，而是`&mut operation`。`operation`变量正在跟踪异步操作。循环的每次迭代都会执行相同的操作，而不是对`action()`发出新调用。

`select!`的另外一个分支从管道中接收消息，如果接收到了偶数，就跳出循环，否则继续再次执行`select!`。

这是我们第一次使用`tokio::pin!`。我们不打算深入讨论pin的细节。需要注意的是，对一个引用调用`.await`，引用的值必须是pinned或者实现了`Unpin`。

如果我们删掉`tokio::pin!`，尝试编译就会得到以下错误：

```
error[E0599]: no method named `poll` found for struct
     `std::pin::Pin<&mut &mut impl std::future::Future>`
     in the current scope
  --> src/main.rs:16:9
   |
16 | /         tokio::select! {
17 | |             _ = &mut operation => break,
18 | |             Some(v) = rx.recv() => {
19 | |                 if v % 2 == 0 {
...  |
22 | |             }
23 | |         }
   | |_________^ method not found in
   |             `std::pin::Pin<&mut &mut impl std::future::Future>`
   |
   = note: the method `poll` exists but the following trait bounds
            were not satisfied:
           `impl std::future::Future: std::marker::Unpin`
           which is required by
           `&mut impl std::future::Future: std::future::Future`
```

虽然我们在[上一章](https://tokio.rs/tokio/tutorial/async)介绍了`Future`，也不能很清晰的解释这个错误。如果你在尝试对**引用**调用`.await`遇到了有关于`Future`未实现的错误，那么你可能需要pin future。

在[标准库](https://doc.rust-lang.org/std/pin/index.html)中读更多有关于[`Pin`](https://doc.rust-lang.org/std/pin/index.html)的内容。

## 设定一个分支 Modifying a branch


让我们看一下稍微复杂一点的循环。我们有：

1. 一管道的`i32`值。
2. 对`i32`值的异步操作。

我们想要实现的逻辑：

1. 等待从管道中接收一个**偶数**。
2. 把这个偶数作为异步操作的输入。
3. 等待操作完成，同时从管道中监听更多的偶数。
4. 如果又接收到一个新偶数，但此时已存在的异步操作未完成，打断这个异步操作，并传入新偶数开始新的该异步操作。

```rust
async fn action(input: Option<i32>) -> Option<String> {
    // 输入输入是 `None`，返回 `None`。
    // 也可以这样写 `let i = input?;`
    let i = match input {
        Some(input) => input,
        None => return None,
    };
    // 这里是一些异步逻辑
}

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(128);
    
    let mut done = false;
    let operation = action(None);
    tokio::pin!(operation);
    
    tokio::spawn(async move {
        let _ = tx.send(1).await;
        let _ = tx.send(3).await;
        let _ = tx.send(2).await;
    });
    
    loop {
        tokio::select! {
            res = &mut operation, if !done => {
                done = true;

                if let Some(v) = res {
                    println!("GOT = {}", v);
                    return;
                }
            }
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    // `.set` 是 `Pin` 的一个方法。
                    operation.set(action(Some(v)));
                    done = false;
                }
            }
        }
    }
}
```

我们使用了与之前相似的策略。异步函数在循环外被调用，并且给了`operation`。`operation`变量被pinned。选择循环体作用在`operation`和通道的接收者上。

注意现在`action`接收`Option<i32>`参数。当我们接收到第一个偶数前，我们需要实例化`operation`为某些东西。我们让`action`接收`Option`并返回`Option`。如果传进来了`None`，那就返回`None`。第一次循环迭代`operation`会立刻返回`None`。

此示例用了一些新语法。第一个分支有一个`, if !done`。这是该分支的一个前提条件。在解析它是如何工作之前，让我们看看省略该前提条件会发生什么。删掉`, if !done`并运行该示例，会导致以下输出：

```
thread 'main' panicked at '`async fn` resumed after completion', src/main.rs:1:55
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

当我们尝试在它已经完成后使用`operation`时，发生此错误。通常来说，当使用`.await`时，调用了await的值会被消耗。在这个例子中，我们等待一个引用。这意味着`operation`完成后仍然存在。

为了避免panic，我们必须在操作完成后禁用第一个分支。`done`变量用于跟踪`operation`是否完成。一个`select!`分支可以包含一个**前提条件**。这个前提条件会在`select!`在该分支上调用await**之前**检查。如果条件是`false`，那么该分支被禁用。通常，`done`变量被初始化为`false`。当`operation`完成之后，`done`被设置为`true`。下一次循环迭代就会禁用这个`operation`分支。当从管道中接收到一个偶数时，`operation`被重置，`done`被设置为`false`。

# 每个任务的并发方式 Per-task concurrency

`tokio::spawn`和`select!`都可以启用并发异步操作。然而，用于运行并发操作的策略有所不同。`tokio::spawn`函数取得一个异步操作，然后生成一个任务来运行它。任务是Tokio运行时调度的对象。Tokio会独立安排两个不同的任务。这或许会让它们同时运行在两个不同的操作系统线程上。正因如此，生成的任务与生成的线程有相同的限制：不能借用外部的值。

`select!`宏会在**同一个任务上**并发运行所有分支。因为`select!`宏的所有分支在同一个任务上运行，它们永远不会**同时**运行。`select!`宏会在单个任务上多路复用异步操作。