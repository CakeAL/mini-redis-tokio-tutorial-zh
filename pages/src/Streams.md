# Streams 流

流是一系列异步的值。它相当于[`std::iter::Iterator`](https://doc.rust-lang.org/book/ch13-02-iterators.html)的异步版本，并且由[`Stream`](https://docs.rs/futures-core/0.3/futures_core/stream/trait.Stream.html) trait 来表示。流可以在`异步`函数中迭代。它们也可以通过适配器(adapter)来转换。Tokio 在[`StreamExt`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html) trait 上提供了许多通用的适配器。

Tokio 通过一个独立的 crate`tokio-stream`提供了流支持。

```toml
tokio-stream = "0.1"
```

> **info**
> 当前，Tokio 的流工具包存在于`tokio-stream`crate 中。一旦`Stream` trait 在 Rust 的标准库中稳定下来，Tokio 的流工具包将会迁移到`tokio` crate。

# 迭代

当前，Rust 不支持异步`for`循环。取而代之的是，使用`while let`循环搭配[`StreamExt::next()`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.next)来迭代流。

```rust
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let mut stream = tokio_stream::iter(&[1, 2, 3]);

    while let Some(v) = stream.next().await {
        println!("GOT = {:?}", v);
    }
}
```

就像迭代器一样，`next()`方法返回`Option<T>`，`T`是流的值类型。接收到`None`表示流迭代终止。

## Mini-Redis 的广播

让我们看一下使用 Mini-Redis 的稍微复杂点的示例。

完整代码可在[这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/streams/src/main.rs)找到。

```rust
use tokio_stream::StreamExt;
use mini_redis::client;

async fn publish() -> mini_redis::Result<()> {
    let mut client = client::connect("127.0.0.1:6379").await?;

    // 发布一些数据
    client.publish("numbers", "1".into()).await?;
    client.publish("numbers", "two".into()).await?;
    client.publish("numbers", "3".into()).await?;
    client.publish("numbers", "four".into()).await?;
    client.publish("numbers", "five".into()).await?;
    client.publish("numbers", "6".into()).await?;
    Ok(())
}

async fn subscribe() -> mini_redis::Result<()> {
    let client = client::connect("127.0.0.1:6379").await?;
    let subscriber = client.subscribe(vec!["numbers".to_string()]).await?;
    let messages = subscriber.into_stream();

    tokio::pin!(messages);

    while let Some(msg) = messages.next().await {
        println!("got = {:?}", msg);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> mini_redis::Result<()> {
    tokio::spawn(async {
        publish().await
    });

    subscribe().await?;

    println!("DONE");

    Ok(())
}
```

生成一个任务来向 Mini-Redis 服务端发布消息到"numbers"频道上。然后，在 main 任务中，我们订阅了"numbers"频道，并且显示接收到的消息。

订阅后，[`into_stream()`](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Subscriber.html#method.into_stream)被调用，返回了订阅者(subscriber)。这会消耗`订阅者`，返回一个流，该流会在消息到达时生成消息。在我们开始迭代消息之前，注意流通过[`tokio::pin!`](https://docs.rs/tokio/1/tokio/macro.pin.html)被[pin](https://doc.rust-lang.org/std/pin/index.html)到了栈上。在流上调用`next()`需要这个流被 pin。`into_stream()`返回了一个*没有*pin 的流，我们必须显式 pin 它才能迭代他。

> **info**  
> 当一个 Rust 值在内存中无法再被移动，就说是被“pin”。一个 pinned 的值的关键属性就是指针可以指向 pinned 的数据，并且调用者可以确信指针可以一直有效。`async/await`使用这个特性来支持跨`.await`借用数据(borrowing data across `.await` points)。

如果我们忘了 pin 流，我们会得到以下错误：

```
error[E0277]: `from_generator::GenFuture<[static generator@Subscriber::into_stream::{closure#0} for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {ResumeTy, &'r mut Subscriber, Subscriber, impl Future, (), std::result::Result<Option<Message>, Box<(dyn std::error::Error + Send + Sync + 't0)>>, Box<(dyn std::error::Error + Send + Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't4)>>>, std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't5)>>, impl Future, Option<Message>, Message}]>` cannot be unpinned
  --> streams/src/main.rs:29:36
   |
29 |     while let Some(msg) = messages.next().await {
   |                                    ^^^^ within `tokio_stream::filter::_::__Origin<'_, impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>`, the trait `Unpin` is not implemented for `from_generator::GenFuture<[static generator@Subscriber::into_stream::{closure#0} for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {ResumeTy, &'r mut Subscriber, Subscriber, impl Future, (), std::result::Result<Option<Message>, Box<(dyn std::error::Error + Send + Sync + 't0)>>, Box<(dyn std::error::Error + Send + Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't4)>>>, std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 't5)>>, impl Future, Option<Message>, Message}]>`
   |
   = note: required because it appears within the type `impl Future`
   = note: required because it appears within the type `async_stream::async_stream::AsyncStream<std::result::Result<Message, Box<(dyn std::error::Error + Send + Sync + 'static)>>, impl Future>`
   = note: required because it appears within the type `impl Stream`
   = note: required because it appears within the type `tokio_stream::filter::_::__Origin<'_, impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>`
   = note: required because of the requirements on the impl of `Unpin` for `tokio_stream::filter::Filter<impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>`
   = note: required because it appears within the type `tokio_stream::map::_::__Origin<'_, tokio_stream::filter::Filter<impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>, [closure@streams/src/main.rs:26:14: 26:40]>`
   = note: required because of the requirements on the impl of `Unpin` for `tokio_stream::map::Map<tokio_stream::filter::Filter<impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>, [closure@streams/src/main.rs:26:14: 26:40]>`
   = note: required because it appears within the type `tokio_stream::take::_::__Origin<'_, tokio_stream::map::Map<tokio_stream::filter::Filter<impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>, [closure@streams/src/main.rs:26:14: 26:40]>>`
   = note: required because of the requirements on the impl of `Unpin` for `tokio_stream::take::Take<tokio_stream::map::Map<tokio_stream::filter::Filter<impl Stream, [closure@streams/src/main.rs:22:17: 25:10]>, [closure@streams/src/main.rs:26:14: 26:40]>>`
```

如果你得到像这样的错误信息，尝试 pin 那个值！

当你尝试运行这个，首先开启 Mini-Redis 服务端：

```bash
$ mini-redis-server
```

尝试运行代码。我们会在 STDOUT 得到以下输出。

```
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"two" })
got = Ok(Message { channel: "numbers", content: b"3" })
got = Ok(Message { channel: "numbers", content: b"four" })
got = Ok(Message { channel: "numbers", content: b"five" })
got = Ok(Message { channel: "numbers", content: b"6" })
```

由于订阅和发布之间存在竞争，一些早期的消息可能被 drop。该程序永远不会退出。只要服务端保持活动状态，对 Mini-Redis 频道的订阅就会保持活动状态。

让我们看看如何使用流扩展这个程序。

# 适配器

接收一个流病返回其他流的函数被叫做“流适配器”，因为它们是“适配器模式”的一种形式。常见的流适配器包含 map, take 和 filter。

让我们更新 Mini-Redis 代码来让它可以退出。在接收到三条消息之后，停止迭代消息。这是使用[`take`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.take)完成的。这个适配器限制流**最多**产生`n`条消息。

```rust
let messages = subscriber
    .into_stream()
    .take(3);
```

再次运行程序，我们看到以下输出：

```
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"two" })
got = Ok(Message { channel: "numbers", content: b"3" })
```

这次，程序会停止了。

现在，让我们限制流只返回一位十进制数字。我们将会通过检查消息长度来检查这一点。我们使用[`filter`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.filter)适配器来 drop 其他不匹配的消息。

```rust
let messages = subscriber
    .into_stream()
    .filter(|msg| match msg {
        Ok(msg) if msg.content.len() == 1 => true,
        _ => false,
    })
    .take(3);
```

再次运行程序，我们看到以下输出：

```
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"3" })
got = Ok(Message { channel: "numbers", content: b"6" })
```

注意，应用适配器的顺序很重要。先调用`filter`，然后`take`，跟先调用`take`，再调用`filter`效果是不同的。

最后，我们来整理`Ok(Message { ... })`输出。这是通过`map`完成的。因为这是在`filter`**之后**执行的，所以我们知道消息肯定是`Ok`的，所以我们可以用`unwrap()`。

```rust
let messages = subscriber
    .into_stream()
    .filter(|msg| match msg {
        Ok(msg) if msg.content.len() == 1 => true,
        _ => false,
    })
    .map(|msg| msg.unwrap().content)
    .take(3);
```

现在，输出是：

```
got = b"1"
got = b"3"
got = b"6"
```

另一种选择是组合[`filter`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.filter)和[`map`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.map)到单次调用[`filter_map`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.filter_map)中。

这里还有更多可用适配器。查看[这个列表](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html)。

# 实现`流`

[`Stream`](https://docs.rs/futures-core/0.3/futures_core/stream/trait.Stream.html)trait 与[`Future`](https://doc.rust-lang.org/std/future/trait.Future.html)trait 非常相似。

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Stream {
    type Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
```

`Stream::poll_next()`函数非常像`Future::poll`，不同之处在于它可以重复调用从流中接收许多值。正如我们在[深入异步章节](https://tokio.rs/tokio/tutorial/async)中看到的那样，当一个流**没**准备好返回值，就会返回`Poll::Pending`。任务的 waker 已经注册，一旦流准备好被再次轮询，waker 就会收到通知。

`size_hint()`方法与[迭代器](https://doc.rust-lang.org/book/ch13-02-iterators.html)一样。

通常来说，手动实现`Stream`时，是通过组合 future 和其他流来完成的。例如，在深入异步章节中构建`Delay` future 时。我们可以把它转换为一个流，间隔 10ms，生成三次`()`。

```rust
use tokio_stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

struct Interval {
    rem: usize,
    delay: Delay,
}

impl Interval {
    fn new() -> Self {
        Self {
            rem: 3,
            delay: Delay { when: Instant::now() }
        }
    }
}

impl Stream for Interval {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<()>>
    {
        if self.rem == 0 {
            // 不用再延迟了
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.delay).poll(cx) {
            Poll::Ready(_) => {
                let when = self.delay.when + Duration::from_millis(10);
                self.delay = Delay { when };
                self.rem -= 1;
                Poll::Ready(Some(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
```

`async-stream`异步流

但是手动实现[`Stream`](https://docs.rs/futures-core/0.3/futures_core/stream/trait.Stream.html) trait 可能很无聊。不幸的是，Rust 尚不支持`async/await`语法来定义流。这正在开发中，但并没有就绪。

`async-stream`crate 可作为临时解决方案。这个 crate 提供了`stream!`宏来转换输入的流。通过使用这个 crate，上面的间隔返回可以这样实现：

```rust
use async_stream::stream;
use std::time::{Duration, Instant};

stream! {
    let mut when = Instant::now();
    for _ in 0..3 {
        let delay = Delay { when };
        delay.await;
        yield ();
        when += Duration::from_millis(10);
    }
}
```
