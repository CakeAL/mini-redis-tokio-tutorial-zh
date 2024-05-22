# Framing 解析帧

现在我们将为 Mini-Redis 框架层实现我们刚刚学过的 I/O 知识。获取字节流，并把它转换为帧流的过程叫解析帧。一帧就是两个对等点(peer)之间的传输单元。Redis 协议帧定义如下：

```rust
use bytes::Bytes;

enum Frame {
    Simple(String),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>),
}
```

注意观察该帧仅由数据组成，没有任何语义。指令的解析和执行发生在更高的层次。

对于 HTTP 来说，一帧可能长这样：

```rust
enum HttpFrame {
    RequestHead {
        method: Method,
        uri: Uri,
        version: Version,
        headers: HeaderMap,
    },
    ResponseHead {
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
    },
    BodyChunk {
        chunk: Bytes,
    },
}
```

为了给 Mini-Redis 实现数据帧，我们将先实现`Connection`结构体，包含了`TcpStream`和读写`mini_redis::Frame`的值。

```rust
use tokio::net::TcpStream;
use mini_redis::{Frame, Result};

struct Connection {
    stream: TcpStream,
    // ... 其他成员变量
}

impl Connection {
    /// 从连接中读取一帧
    ///
    /// 遇到 EOF 返回 `None`
    pub async fn read_frame(&mut self)
        -> Result<Option<Frame>>
    {
        // 在这里实现
    }

    /// 向连接中写入一帧
    pub async fn write_frame(&mut self, frame: &Frame)
        -> Result<()>
    {
        // 在这里实现
    }
}
```

你可以在[这儿](https://redis.io/topics/protocol)找到有关 Redis 有线协议的详情。完整的`Connection`代码在[这儿](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs)。

# 读取缓冲区

`read_frame`方法在返回前等待接收整个数据帧。对`TcpStream::read()`单次调用可能返回任意数量的数据。它可能有一整个帧，一部分帧或者多个帧。如果只接收到一部分帧，则传到缓冲区，再从套接字读取更多数据。如果接收到多个帧，则返回第一帧，其他数据存到缓冲区，直到下次调用`read_frame`。

如果还没创建`connection.rs`，这样创建：

```bash
touch src/connection.rs
```

为了实现这种功能，连接(Connection)需要一个读缓冲区。从套接字读取到数据会存到读缓冲区。当解析了一帧时，缓冲区中相应的数据就会删除。

我们将使用[`BytesMut`](https://docs.rs/bytes/1/bytes/struct.BytesMut.html)来作为缓冲区类型，它是[`Bytes`](https://docs.rs/bytes/1/bytes/struct.Bytes.html)的可变版本。

```rust
use bytes::BytesMut;
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // 为缓冲区开辟4kb的容量
            buffer: BytesMut::with_capacity(4096),
        }
    }
}
```

接下来，我们实现`read_frame()`方法。

```rust
use tokio::io::AsyncReadExt;
use bytes::Buf;
use mini_redis::Result;

pub async fn read_frame(&mut self)
    -> Result<Option<Frame>>
{
    loop {
        // 尝试从缓冲区解析一个数据帧。如果缓冲区中有足够的数据，
        // 就返回数据帧
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // 缓冲区中没有足够的数据组成一帧。
        // 尝试从套接字中读更多数据。
        //
        // 如果成功，返回字节的数量。
        // 返回 `0` 表示 “读到了流的末尾”
        if 0 == self.stream.read_buf(&mut self.buffer).await? {
            // 远程连接关闭。对这情况是一种干净的关闭，在读缓冲区中应该
            // 没有数据了。如果还有数据，意味着传输一帧的同时，对等点peer
            // 关闭了套接字。
            if self.buffer.is_empty() {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        }
    }
}
```

分析一下代码。`read_frame`方法在循环体中运行。首先，调用`self.parse_frame()`。这会尝试从`self.buffer`中解析一个 Redis 帧。如果有足够的数据可以解析成一帧，就把该帧返回。否则，尝试从套接字中读取更多数据到缓冲区中。读取到更多数据之后，`parse_frame()`再次被调用。这回，如果接收到足够的数据，解析或许就会成功。

当从流中读取时，返回了`0`表示我们不会从对方接收到更多数据。如果缓冲区中仍有数据，说明是接收到了部分帧，但是连接突然终止了。这是一个错误情况需要返回`Err`。

## `Buf`特征

从流中读取时，调用了`read_buf`，这个读取函数采用了实现[`bytes`](https://docs.rs/bytes/)crate 中[`BufMut`](https://docs.rs/bytes/1/bytes/trait.BufMut.html)的值。

首先，考虑使用`read()`实现相同的读取循环。`Vec<u8>`可以用来替代`BytesMut`。

```rust
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // 为缓冲区开辟4kb的容量
            buffer: vec![0; 4096],
            cursor: 0,
        }
    }
}
```

为`Connection`实现`read_frame()`：

```rust
use mini_redis::{Frame, Result};

pub async fn read_frame(&mut self)
    -> Result<Option<Frame>>
{
    loop {
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // 确保buffer有足够容量
        if self.buffer.len() == self.cursor {
            // 给buffer扩容
            self.buffer.resize(self.cursor * 2, 0);
        }

        // 从流中读取到buffer中，记录读了多少字节
        let n = self.stream.read(
            &mut self.buffer[self.cursor..]).await?;

        if 0 == n {
            if self.cursor == 0 {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        } else {
            // 更新指针
            self.cursor += n;
        }
    }
}
```

当用字节数组`读`时，我们必须维护一个指针，跟踪已使用的缓冲区数量。必须保证把缓冲区的空部分传给`read()`。否则，我们会覆盖已经缓冲了的数据。如果缓冲区满了，必须扩容缓冲区才能继续读取。在`parse_frame()`中（不包括在内），我们需要解析包含在`self.buffer[..self.cursor]`中的数据。

由于将字节数组和指针搭配使用非常常见，`byte`crate 提供了表示字节数组和指针的抽象。`Buf`特征可以被读取数据的类型实现。`BufMut`特征可以被写入数据的类型实现。当把`T: BufMut`传递给`read_buf()`时，缓冲区的内部指针就会由`read_buf`自动更新。正因如此，在我们之前写的`read_frame`中，我们不需要管理自己的指针。

# 解析

现在，让我们实现`parse_frame()`函数。解析由两步组成：

1. 确保缓冲区中有一个完整的帧，并找到帧的末尾索引。
2. 解析帧。

`mini-redis` crate 为每一步都提供了对应的函数：

1. [`Frame::check`](https://docs.rs/mini-redis/0.4/mini_redis/frame/enum.Frame.html#method.check)
2. [`Fream::parse`](https://docs.rs/mini-redis/0.4/mini_redis/frame/enum.Frame.html#method.parse)

我们将会再用`Buf`抽象来获取帮助。`Buf`被传递到`Frame::check`中。`check`会遍历缓冲区，内部指针将会前进。当`check`函数返回时，缓冲区内部指针指向帧的末尾。

对于`Buf`类型，我们使用[`std::io::Cursor<&[u8]>`](https://doc.rust-lang.org/stable/std/io/struct.Cursor.html)

```rust
use mini_redis::{Frame, Result};
use mini_redis::frame::Error::Incomplete;
use bytes::Buf;
use std::io::Cursor;

fn parse_frame(&mut self)
    -> Result<Option<Frame>>
{
    // 创建 `T: Buf` 类型.
    let mut buf = Cursor::new(&self.buffer[..]);

    // 检查是否已有一个完整的帧
    match Frame::check(&mut buf) {
        Ok(_) => {
            // 获取帧的字节长度
            let len = buf.position() as usize;

            // 重置内部指针位置，因为调用了 `parse`
            buf.set_position(0);

            // 解析帧
            let frame = Frame::parse(&mut buf)?;

            // 丢弃缓冲区中的帧
            self.buffer.advance(len);

            // 返回解析后的帧
            Ok(Some(frame))
        }
        // 缓冲区中没有足够数据
        Err(Incomplete) => Ok(None),
        // 遇到了错误
        Err(e) => Err(e.into()),
    }
}
```

完整的[`Frame::check`](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/frame.rs#L65-L103)函数可以在[这儿](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/frame.rs#L65-L103)找到。这里不会完整介绍它。

需要注意的是，相关事项使用了`Buf`的“字节迭代器”风格 API。它们获取数据并移动内部指针。例如，解析一帧，第一个字节被检查来确定类型。使用了[`Buf::get_u8`](https://docs.rs/bytes/1/bytes/buf/trait.Buf.html#method.get_u8)函数。这会获取到当前指针位置的字节并让指针前进一次。

[`Buf`](https://docs.rs/bytes/1/bytes/buf/trait.Buf.html)特征也有很多其他实用方法。查看[API 文档](https://docs.rs/bytes/1/bytes/buf/trait.Buf.html)获取更多细节。

# 缓冲写入(Buffered writes)

解析帧的另一部分就是`write_frame(frame)`函数。这个函数把整个帧写入套接字。为了最小地调用`write`，我们使用缓冲写入。维护一个写入缓冲区，帧在写入套接字之前需要先编码到这个缓冲区。然而，不像`read_frame()`，整个帧并不总是缓存到字节数组中。

考虑到一些流中的帧。正在写入的值是`Frame::Bulk(Bytes)`。这些帧线性排列，有一个帧头，它由`$`符后跟了整个数据长度那么多的字节。帧大部分内容都是`Bytes`值。如果数据很大，把它复制到缓冲区的成本很高。

为了实现缓冲写入，我们需要[`BufWriter`结构体](https://docs.rs/tokio/1/tokio/io/struct.BufWriter.html)。该结构体使用`T: AsyncWrite`初始化并实现了`AsyncWrite`。当在`BufWriter`上调用`write`时，写入不会直接写入writer，而是写入缓冲区。当缓冲区满了，内容将会被刷新到writer，然后缓冲区清空。还有一些特殊的优化用来绕过缓冲区，在特定的情况下。

在本教程，我们不会实现完整的`write_frame()`。完整实现在[这儿](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs#L159-L184)。

首先，更新`Connection`结构体：

```rust
use tokio::io::BufWriter;
use tokio::net::TcpStream;
use bytes::BytesMut;

pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(4096),
        }
    }
}
```

接下来，实现`write_frame()`：

```rust
use tokio::io::{self, AsyncWriteExt};
use mini_redis::Frame;

async fn write_frame(&mut self, frame: &Frame)
    -> io::Result<()>
{
    match frame {
        Frame::Simple(val) => {
            self.stream.write_u8(b'+').await?;
            self.stream.write_all(val.as_bytes()).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Error(val) => {
            self.stream.write_u8(b'-').await?;
            self.stream.write_all(val.as_bytes()).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Integer(val) => {
            self.stream.write_u8(b':').await?;
            self.write_decimal(*val).await?;
        }
        Frame::Null => {
            self.stream.write_all(b"$-1\r\n").await?;
        }
        Frame::Bulk(val) => {
            let len = val.len();

            self.stream.write_u8(b'$').await?;
            self.write_decimal(len as u64).await?;
            self.stream.write_all(val).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Array(_val) => unimplemented!(),
    }

    self.stream.flush().await;

    Ok(())
}
```

这里使用的函数由[`AsyncWriteExt`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html)提供。它们也能用在`TcpStream`，但是不建议在没有中间缓冲区的情况下处理单个字节。

- [`write_u8`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html#method.write_u8)向writer写入一个字节。
- [`write_all`](https://docs.rs/tokio/1/tokio/io/trait.AsyncWriteExt.html#method.write_all)向writer写入全部。
- [`write_decimal`](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs#L225-L238)是mini-redis实现的方法。

该函数以调用`self.stream.flush().await`结束。因为`BufWriter`向中间缓冲区存储了需要写入的内容，所以调用`write`不能保证数据被写入套接字。Return之前，我们想要帧被写入到套接字中。调用`fluse()`会将在缓冲区挂起的数据写入到套接字中。

另一种选择是**不在**`write_frame()`中调用`flush()`。取而代之的是，在`Connection`中提供`flush()`函数。这将允许调用者写入多个小帧到写缓冲区里，然后通过`write`系统调用，把他们写入到套接字中。这样会导致`Connection`API变得复杂。所以我们决定在`fn write_frame()`中调用`fluse().await`。