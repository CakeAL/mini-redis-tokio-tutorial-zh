# I/O

Tokio 中的 I/O 操作，与`std`中差不多，但是是异步的。又一个用来读的 trait [`AsyncRead`](https://docs.rs/tokio/1/tokio/io/trait.AsyncRead.html) 和一个用来写的 trait [`AsyncWrite`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWrite.html)。这些类型都实现了上述 trait([`TcpStream`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html), [`File`](https://docs.rs/tokio/1/tokio/fs/struct.File.html), [`Stdout`](https://docs.rs/tokio/1/tokio/io/struct.Stdout.html))。[`AsyncRead`](https://docs.rs/tokio/1/tokio/io/trait.AsyncRead.html)和[`AsyncWrite`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWrite.html)也为一些数据结构实现了，比如`Vec<u8>`和`&[u8]`。这就可以让在读写的时候使用字节数组。

本章节将展示几个例子介绍通过 Tokio 进行 I/O 读写。下一章会介绍更高级的 I/O 示例。

# `AsyncRead`和`AsyncWrite`

这俩 trait 都提供了异步读写比特流的方法。这些 trait 上的方法通常不能直接调用，就像你不能直接从`Future`trait 中手动调用`poll`方法一样。但是，你仍然可以使用[`AsyncReadExt`](https://docs.rs/tokio/1/tokio/io/trait.AsyncReadExt.html)和[`AsyncWriteExt`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html)提供的实用方法来使用他们。

让我们简要看一下这些方法。这些函数都是`异步的`并且他们必须与`.await`搭配使用。

`async fn read()`

[`AsyncReadExt::read()`](https://docs.rs/tokio/1/tokio/io/trait.AsyncReadExt.html#method.read)提供了读取数据到缓冲区的异步方法，返回读取的字节数。

**注意：**当`read()`返回了`Ok(0)`，这标志着流已经关闭了。再对`read()`调用都会立刻返回`Ok(0)`。对于[`TcpStream`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html)来说，这表示对套接字的读取部分已经关闭。

```rust
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = [0; 10];

    // 最多读取10字节
    let n = f.read(&mut buffer[..]).await?;

    println!("The bytes: {:?}", &buffer[..n]);
    Ok(())
}
```

`async fn read_to_end()`

[`AsyncReadExt::read_to_end`](https://docs.rs/tokio/1/tokio/io/trait.AsyncReadExt.html#method.read_to_end)读取流中所有的字节，直到 EOF。

```rust
use tokio::io::{self, AsyncReadExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = Vec::new();

    // 读取整个文件
    f.read_to_end(&mut buffer).await?;
    Ok(())
}
```

`async fn write()`

[`AsyncWriteExt::write`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html#method.write)将缓冲区写入 writer，返回写入的字节数。

```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    // 写入了这个字节字符串的前缀一部分，不是所有的都必须写入
    let n = file.write(b"some bytes").await?;

    println!("Wrote the first {} bytes of 'some bytes'.", n);
    Ok(())
}
```

`async fn write_all()`

[`AsyncWriteExt::write_all`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html#method.write_all)将缓冲区所有字节都写入 writer。

```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    file.write_all(b"some bytes").await?;
    Ok(())
}
```

这两个特征还有其他很多有用的方法。查看 API 文档以获取完整列表。

# 辅助函数 (Helper functions)

另外，就像`std`一样，[`tokio::io`](https://docs.rs/tokio/1/tokio/io/index.html)也提供了很多实用的辅助函数，比如处理[`standard input`](https://docs.rs/tokio/1/tokio/io/fn.stdin.html)，[`standard output`](https://docs.rs/tokio/1/tokio/io/fn.stdout.html)和[`standard error`](https://docs.rs/tokio/1/tokio/io/fn.stderr.html)的 API。例如，`tokio::io::copy`可以异步地复制 reader 中的全部内容到 writer 中。

```rust
use tokio::fs::File;
use tokio::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut reader: &[u8] = b"hello";
    let mut file = File::create("foo.txt").await?;

    io::copy(&mut reader, &mut file).await?;
    Ok(())
}
```

注意，这里的字节数组也实现了`AsyncRead`。

# 回声服务器(Echo server)

让我们联系一下使用异步 I/O。我们将编写一个回声服务器。

回声服务器会绑定一个`TcpListener`并循环接收入站连接。对每个入站连接，从套接字中读取数据，然后立即写回到套接字中。客户端向服务端发送数据，并接收返回的相同数据。

我们将使用不同方式来实现回声服务器两次。

## 使用`io::copy()`

首先，我们将用`io::copy()`工具来实现回声逻辑。

你可以在一个新二进制文件中写代码：

```bash
$ touch src/bin/echo-server-copy.rs
```

然后你可以这样运行它（或仅仅是检查编译是否成功）：

```bash
$ cargo run --bin echo-server-copy
```

你可以用标准命令行工具（比如 telnet）来测试这个服务端，或者写一个简单的客户端（比如[`tokio::net::TcpStream`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html#examples)文档中的客户端）。

下面代码是一个 TCP 服务端，并有一个接收循环体。每当传入一个套接字，都会生成一个新任务。

```rust
use tokio::io;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            // 在这里复制数据。
        });
    }
}
```

刚刚说过，实用函数会需要一个 reader 和一个 writer，然后把数据从一个复制到另一个。然而，我们只有一个`TcpStream`，它同时实现了`AsyncRead`和`AsyncWrite`。因为`io::copy`对于 reader 和 writer 都需要`&mut`，所以套接字不能同时应用于两个参数。

```rust
// 这不能编译
io::copy(&mut socket, &mut socket).await
```

## 把 reader + writer 分开

为了解决此问题，我们需要把套接字拆分为 reader 句柄和 writer 句柄。拆分 reader/writer 组合最好的方法取决于具体的类型。

任何 reader + writer 类型都需要使用[`io::split`](https://docs.rs/tokio/1/tokio/io/fn.split.html)工具拆分。这个函数传入单个值，并返回拆分后的 reader 和 writer。这样两个句柄就可以单独使用，也可在单独的任务中使用。

例如，回声客户端可以这样并发处理读写：

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = TcpStream::connect("127.0.0.1:6142").await?;
    let (mut rd, mut wr) = io::split(socket);

    // 后台写入数据
    tokio::spawn(async move {
        wr.write_all(b"hello\r\n").await?;
        wr.write_all(b"world\r\n").await?;

        // 有时，Rust类型推断需要点小帮助
        Ok::<_, io::Error>(())
    });

    let mut buf = vec![0; 128];

    loop {
        let n = rd.read(&mut buf).await?;

        if n == 0 {
            break;
        }

        println!("GOT {:?}", &buf[..n]);
    }

    Ok(())
}
```

由于`io::split`支持**任何**实现了`AsyncRead + AsyncWrite`的类型，并返回独立的句柄，这会导致在`io::split`内部使用`Arc`和`Mutex`。使用`TcpSteam`可以避免这种开销。`TcpSteam`提供了两个专门的分割函数。

[`TcpSteam::split`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html#method.split)获取了对流的**引用**，并返回一个 reader 和 writer 句柄。因为使用了引用，两个句柄都必须保持在和调用`split()`**同样的**任务上。这种特殊的分割是零成本的。因为不需要`Arc`或`Mutex`。`TcpStream`也提供了[`into_split`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html#method.into_split)，来支持可以在任务间移动的句柄，代价仅仅是一个`Arc`。

因为是在拥有`TcpStream`同一个任务上调用`io::copy()`，所以我们可以使用[`TcpStream::split`](https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html#method.split)。服务端中处理回声逻辑的任务应该这样写：

```rust
tokio::spawn(async move {
    let (mut rd, mut wr) = socket.split();

    if io::copy(&mut rd, &mut wr).await.is_err() {
        eprintln!("failed to copy");
    }
});
```

在[这儿](https://github.com/tokio-rs/website/blob/master/tutorial-code/io/src/echo-server-copy.rs)可以找到完整代码。

## 手动拷贝

现在，让我们看看如何手动拷贝数据来编写回声服务器。为此，我们使用[`AsyncReadExt::read`](https://docs.rs/tokio/1/tokio/io/trait.AsyncReadExt.html#method.read)和[`AsyncWriteExt::write_all`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html#method.write_all)。

完整的回声服务器代码：

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = vec![0; 1024];

            loop {
                match socket.read(&mut buf).await {
                    // 返回值为 `Ok(0)` 的话说明远程关闭了
                    Ok(0) => return,
                    Ok(n) => {
                        // 拷贝数据返回到套接字中
                        if socket.write_all(&buf[..n]).await.is_err() {
                            // 意料之外的套接字错误，我们不能为此干什么，
                            // 只能把服务先关了
                            return;
                        }
                    }
                    Err(_) => {
                        // 意料之外的套接字错误，我们不能为此干什么，
                        // 只能把服务先关了
                        return;
                    }
                }
            }
        });
    }
}
```

（你可以把这些代码放到`src/bin/echo-server.rs`里，然后通过`cargo run --bin echo-server`来启动它）

让我们分析一下代码。首先，由于使用了`AsyncRead`和`AsyncWrite`，所以必须 use 一下。

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
```

## 分配缓冲区

这是为了从套接字中读取一些数据到缓冲区，然后将缓冲区的内容写回到套接字中。

```rust
let mut buf = vec![0; 1024];
```

避免使用在栈上的缓冲区。[`回想一下`](https://tokio.rs/tokio/tutorial/spawning#send-bound)，注意到跨`.await`调用的任务数据都需要由任务来存储。这种情况下，`buf`是用来跨`.await`调用使用的。所有任务数据都必须存储在单个分配中。你可以把它看成一个`枚举`，其中每个枚举值都是特定调用`.await`时需要存储的数据。

如果缓冲区在栈上，每个传入的套接字生成的任务内部结构可能类似于：

```rust
struct Task {
    // 任务内部成员
    task: enum {
        AwaitingRead {
            socket: TcpStream,
            buf: [BufferType],
        },
        AwaitingWriteAll {
            socket: TcpStream,
            buf: [BufferType],
        }

    }
}
```

如果缓冲区在栈上，那么它将*内连*在任务结构体中。这会导致任务结构非常庞大。进一步说，缓冲区大小通常是页那么大。反过来，会导致`任务`大小很尴尬：`$page-size + a-few-bytes`。

相对于基本`枚举`来说，编译器对此也优化了异步块的布局。实际上，变量不会像`枚举`那样在变量变体之间移动。然而，任务结构体的大小至少与最大变量一样大。

正因如此，通常更高效的方法就是为缓冲区在堆上分配内存。

## 处理 EOF

当 TCP 流的读取部分关闭，调用`read()`会返回`Ok(0)`。此时退出循环非常重要。忘记在读到 EOF 时退出循环是常见的错误。

```rust
loop {
    match socket.read(&mut buf).await {
        // 返回了 `Ok(0)` 意味着远程连接关闭了
        Ok(0) => return,
        // ... 这里处理其他情况
    }
}
```

忘记从读循环中退出，通常会导致 100% CPU 无限循环。套接字关闭，`socket.read()`会立刻返回。然后就死循环了。

[这儿](https://github.com/tokio-rs/website/blob/master/tutorial-code/io/src/echo-server.rs)可以找到完整代码。
