# Unit Testing 单元测试

本页的目的是提供有关如何在异步应用中写单元测试的建议。

# 测试中暂停和恢复时间

有时，异步代码可以通过[`tokio::time::sleep`](https://docs.rs/tokio/1/tokio/time/fn.sleep.html)或等待[`tokio::time::Interval::tick`](https://docs.rs/tokio/1/tokio/time/struct.Interval.html#method.tick)来显式等待。当单元测试开始运行的非常缓慢时，基于时间的测试（例如，指数避退）可能变得非常麻烦。然而，在内部 tokio 的时间相关功能支持暂停和恢复时间。任何提前准备好的时间相关的 future 都有暂停时间的效果。与时间相关的 future 的提前解决的条件是没有更多其他可能提前就绪的 future。当唯一的 future 与时间相关时，这在本质上是时间的快进：

```rust
#[tokio::test]
async fn paused_time() {
    tokio::time::pause();
    let start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("{:?}ms", start.elapsed().as_millis());
}
```

这代码在正常的机器上会输出`0ms`。

对于单元测试来说，整个过程中保持时间暂停通常很有用。可以通过调用`start_paused`宏设置为`true`来实现：

```rust
#[tokio::test(start_paused = true)]
async fn paused_time() {
    let start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("{:?}ms", start.elapsed().as_millis());
}
```

查看 [tokio::test "设置运行时来让时间暂停"](https://docs.rs/tokio/latest/tokio/attr.test.html#configure-the-runtime-to-start-with-time-paused) 以获取详细信息。

当然，即使使用不同时间相关 future，未来解析的时间顺序也不会改变：

```rust
#[tokio::test(start_paused = true)]
async fn interval_with_paused_time() {
    let mut interval = interval(Duration::from_millis(300));
    let _ = timeout(Duration::from_secs(1), async move {
        loop {
            interval.tick().await;
            println!("Tick!");
        }
    })
    .await;
}
```

这段代码正好打印 4 次`Tick!`。

# 使用[`AsyncRead`](https://docs.rs/tokio/latest/tokio/io/trait.AsyncRead.html)和[`AsyncWrite`](https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html)进行模拟

异步读写的通用 trait（[`AsyncRead`](https://docs.rs/tokio/latest/tokio/io/trait.AsyncRead.html)和[`AsyncWrite`](https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html)）是用来被实现的，比如，套接字。它们可以用于模拟套接字执行的 I/O。

让我们考虑一个 TCP 服务端循环：

```rust
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            eprintln!("Failed to accept client");
            continue;
        };

        tokio::spawn(async move {
            let (reader, writer) = socket.split();
            // 运行一些客户端连接句柄, 比如:
            // handle_connection(reader, writer)
                // .await
                // .expect("Failed to handle connection");
        });
    }
}
```

这里，每个 TCP 客户端连接都由专用的 tokio 任务提供了服务。该任务有一个 reader 和一个 writer，它们是从[`TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html)分离([split](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html#method.split))出来的。

考虑实际客户端句柄任务，尤其是函数签名的**where**句：

```rust
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

async fn handle_connection<Reader, Writer>(
    reader: Reader,
    mut writer: Writer,
) -> std::io::Result<()>
where
    Reader: AsyncRead + Unpin,
    Writer: AsyncWrite + Unpin,
{
    let mut line = String::new();
    let mut reader = BufReader::new(reader);

    loop {
        if let Ok(bytes_read) = reader.read_line(&mut line).await {
            if bytes_read == 0 {
                break Ok(());
            }
            writer
                .write_all(format!("Thanks for your message.\r\n").as_bytes())
                .await
                .unwrap();
        }
        line.clear();
    }
}
```

本质上，实现`AsyncRead`和`AsyncWrite`的 reader 和 writer 都是按顺序提供服务的。对于每个给定的行，句柄都会回复`"Thanks for your message"`。

要对客户端的连接句柄进行单元测试，我们需要使用[`tokio_test::io::Builder`](https://docs.rs/tokio-test/latest/tokio_test/io/struct.Builder.html)进行模拟：

```rust
#[tokio::test]
async fn client_handler_replies_politely() {
    let reader = tokio_test::io::Builder::new()
        .read(b"Hi there\r\n")
        .read(b"How are you doing?\r\n")
        .build();
    let writer = tokio_test::io::Builder::new()
        .write(b"Thanks for your message.\r\n")
        .write(b"Thanks for your message.\r\n")
        .build();
    let _ = handle_connection(reader, writer).await;
}
```
