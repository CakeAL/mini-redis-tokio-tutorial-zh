# Async in depth 深入异步

至此，我们已经基本浏览了异步 Rust 和 Tokio。现在我们将深挖 Rust 的异步运行时模型。在本教程的一开始，我们就指出异步 Rust 采用了独特的方式。现在将解释这是什么意思。

# Futures 未来对象/期货

快速看一下这个基本的异步函数。与本教程已经涵盖的内容相比，这不是什么新鲜的。

```rust
use tokio::net::TcpStream;

async fn my_async_fn() {
    println!("hello from async");
    let _socket = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    println!("async TCP operation complete");
}
```

我们调用该函数，并返回一个值。对这个值调用`.await`。

```rust
#[tokio::main]
async fn main() {
    let what_is_this = my_async_fn();
    // 到这行之前，什么也没打印。
    what_is_this.await;
    // 打印出了文字，与套接字建立连接，关闭连接。
}
```

`my_async_fn()`返回了一个 future。Future 是实现了标准库中[`std::future::Future`](https://doc.rust-lang.org/std/future/trait.Future.html)trait 的值。它们是包含了正在异步计算的值。

[`std::future::Future`](https://doc.rust-lang.org/std/future/trait.Future.html)trait 的定义是：

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Self::Output>;
}
```

[关联类型](https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#specifying-placeholder-types-in-trait-definitions-with-associated-types)`Output`是 future 一旦完成后产生的类型。[`Pin`](https://doc.rust-lang.org/std/pin/index.html)类型是 Rust 能够支持在`async`函数中借用(borrow)的方式。查看[标准库](https://doc.rust-lang.org/std/pin/index.html)文档了解细节。

与其他语言实现 future 的方式不同，Rust 的 future 不代表正在后台发生的计算，而是 Rust future**就是**计算本身。Future 的拥有者负责轮询(polling)future 来推进计算过程。这是通过调用`Future::poll`来完成的。

## 实现`Future`

让我们来实现一个简单的 future。这个 future 将会：

1. 等到特定时刻。
2. 向 STDOUT 输出一些文字。
3. 产生一个字符串。

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        } else {
            // 现在忽略这行。
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let future = Delay { when };

    let out = future.await;
    assert_eq!(out, "done");
}
```

## 异步函数作为 Future

在 main 函数中，我们实例化了 future，并在其上调用`.await`。异步函数中，我们可以对任何实现了`Future`的值调用`.await`。确实，调用`异步`函数本就会返回一个实现了`Future`的匿名类型。`async fn main()`其实就会大致生成下面这样：

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

enum MainFuture {
    // 初始化，永不轮询
    State0,
    // 等待 `Delay`，换句话说，就是 `future.await` 那行。
    State1(Delay),
    // future完成了。
    Terminated,
}

impl Future for MainFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<()>
    {
        use MainFuture::*;

        loop {
            match *self {
                State0 => {
                    let when = Instant::now() +
                        Duration::from_millis(10);
                    let future = Delay { when };
                    *self = State1(future);
                }
                State1(ref mut my_future) => {
                    match Pin::new(my_future).poll(cx) {
                        Poll::Ready(out) => {
                            assert_eq!(out, "done");
                            *self = Terminated;
                            return Poll::Ready(());
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                    }
                }
                Terminated => {
                    panic!("future polled after completion")
                }
            }
        }
    }
}
```

Rust future 是个**状态机**。这里，`MainFuture`枚举表示未来可能发生的状态。Future 开始于`State0`状态。当调用`poll`时，future 会尽可能尝试推进其内部状态。如果 future 能够完成，返回`Poll:Ready`，其中包含着该异步计算的输出。

如果 future 无法完成，通常是由于它正在等待未准备好的资源，则返回`Poll::Pending`。接收到`Poll::Pending`后，向调用者表明 future 会在稍后完成，调用者应该稍后再调用`poll`。

我们也发现 future 是由其他 future 组成的。在其他 future 上调用`poll`会导致调用内部 future 的`poll`方法。

# 执行器 Executors

异步 Rust 函数返回 future。Future 必须调用`poll`来推进它们的状态。Future 都是由 future 组成的。那么问题是，谁来对最深层的 future 调用`poll`呢？

回想刚才的内容，要运行异步函数，不是传递给`tokio::spawn`，就是在 main 函数上加上`#[tokio::main]`注解。这可以让外部生成的 future 提交给 Tokio 的执行器。执行器负责在外部 future 上调用`Future::poll`，来驱动着异步计算完成。

## Mini Tokio

为了更好理解他们是怎么组合到一起的，让我们实现我们自己的最小化的 Tokio 版本！完整代码可以在[这儿](https://github.com/tokio-rs/website/blob/master/tutorial-code/mini-tokio/src/main.rs)找到。

```rust
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use futures::task;

fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(10);
        let future = Delay { when };

        let out = future.await;
        assert_eq!(out, "done");
    });

    mini_tokio.run();
}

struct MiniTokio {
    tasks: VecDeque<Task>,
}

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

impl MiniTokio {
    fn new() -> MiniTokio {
        MiniTokio {
            tasks: VecDeque::new(),
        }
    }

    /// 在 mini-tokio 实例上生成一个future
    fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.push_back(Box::pin(future));
    }

    fn run(&mut self) {
        let waker = task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        while let Some(mut task) = self.tasks.pop_front() {
            if task.as_mut().poll(&mut cx).is_pending() {
                self.tasks.push_back(task);
            }
        }
    }
}
```

这可以运行异步块。创建了一个`Delay`实例用于等待所需时间。然而，我们的实现存在一个重大**缺陷**。我们的执行器永远不会休眠。执行器持续不断地用循环轮询**所有的**生成的 future。但大多数时候，future 还没准备好做更多工作，然后又返回了`Poll::Pending`。这个过程会消耗 CPU 周期，降低效率。

理想情况下，我们想要 mini-tokio 只在 future 有进展的时候来轮询一下 future。当任务需要的被阻塞的资源，转变为可以为请求使用时，就应该轮询一下。比如任务想从 TCP 套接字中读取数据，我们只想让它在 TCP 套接字接收到数据时轮询任务。在上述代码中，任务在给定`Instant`(时刻)之前被阻塞。理想状况下，mini-tokio 应该只在这个时刻后来轮询任务。

为了实现这一点，当资源被轮询时，发现资源并没有准备好。一旦资源处于就绪状态，应该发送一个提醒。

# 唤醒者 Wakers

我们之前缺失了 wakers。这是一个当资源准备好继续某些操作时，来通知正在等待的任务的系统。

让我们再看一下`Future::poll`的定义：

```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context)
    -> Poll<Self::Output>;
```

`poll`的`Context`参数中有一个`waker()`方法。该方法返回一个绑定到当前任务的[`Waker`](https://doc.rust-lang.org/std/task/struct.Waker.html)。这个[`Waker`](https://doc.rust-lang.org/std/task/struct.Waker.html)有一个`wake()`方法。调用这个方法会向执行器发出信号，应该安排执行相关的任务。当资源转变为就绪时，调用`wake()`来通知执行器轮询这个任务，来推进整个过程。

## 更新`Delay`的实现

我们可以使用 wakers 来更新`Delay`：

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::thread;

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        } else {
            // 从当前任务获取一个waker的句柄
            let waker = cx.waker().clone();
            let when = self.when;

            // 生成一个定时器线程
            thread::spawn(move || {
                let now = Instant::now();

                if now < when {
                    thread::sleep(when - now);
                }

                waker.wake();
            });

            Poll::Pending
        }
    }
}
```

现在，经过了请求的时间，调用任务就会被通知，然后执行器可以确保该任务再次被调用。下一步就是更新 mini-tokio，来坚挺唤醒通知(wake notifications)。

这里我们的`Delay`实现还是有点问题。我们一会儿再修复。

> **warning**
> 当一个 future 返回`Poll::Pending`时，它**必须**保证 waker 之后某时可以正常被调用。如果忘了这样做，会导致任务被无限期挂起。 \
> 返回`Poll::Pending`后，忘记唤醒任务是常见的 bug。

回想一下我们第一次写的`Delay`。这是 future 的实现：

```rust
impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        } else {
            // Ignore this line for now.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
```

在返回`Poll::Pending`之前，我们调用了`cx.waker().wake_by_ref()`。这是为了满足 future 的定义。通过返回`Poll::Pending`，我们可以向 waker 发出信号。因为我们还没实现定时器线程，所以我们内联地向 waker 发出信号。这样做会让 future 立刻被重新安排执行，但是这个 future 很可能此时并没有准备好去完成。

注意你可以让不必要地向 waker 发出更多信号。在本例中，即使我们还没准备好继续操作，我们也会向 waker 发出信号。除了浪费 CPU 周期之外没毛病。但是，这种特定的实现会导致忙循环(busy loop)。

## 更新 Mini Tokio 代码

接下来是更新 Mini Tokio 来接收 waker 通知。我们只想让任务被唤醒时来执行任务，为此，Mini Tokio 将提供自己的 waker。当调用 waker，关联的任务就会被执行。Mini-Tokio 对 future 轮询时，会把 waker 传递给 future。

更新后的 Mini Tokio 会使用管道来存储计划执行的任务队列。管道可以让任务排队执行在任何线程上。Waker 必须实现了`Send`和`Sync`。

> **info**  
> `Send`和`Sync`是 Rust 提供关于并发的 trait。可以**发送**到不同线程的类型是`Send`的。大多数类型都是`Send`的，但是像`Rc`这样的就不是。可以通过不可变引用**并发**访问的类型是`Sync`的。类型可以是`Send`但不是`Sync`——一个很好的例子是[`Cell`](https://doc.rust-lang.org/std/cell/struct.Cell.html)，它可以通过不可变引用进行修改，这在并发访问是不安全的。 \
> 了解更多细节，可以看[Rust book 中这一章节](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)。

更新 MiniTokio 结构体。

```rust
use std::sync::mpsc;
use std::sync::Arc;

struct MiniTokio {
    scheduled: mpsc::Receiver<Arc<Task>>,
    sender: mpsc::Sender<Arc<Task>>,
}

struct Task {
    // 这儿一会儿再填。
}
```

Waker 是`Sync`的，并且可以被克隆。当调用`wake`时，任务必须被安排执行。为了实现，我们需要有个管道。当我们调用`wake()`时，任务被发送到管道。我们的`Task`结构体将实现唤醒逻辑。为此，它需要包含生成的 future 和管道的发送部分。我们把`Poll`枚举和 future 都放在`TaskFuture`结构体中，以跟踪最新的`Future::poll()`结果，这是处理虚假唤醒(spurious wake-ups)所须的。更多细节在`TaskFuture`的`poll()`中实现。

```rust
use std::sync::{Arc, Mutex};

/// future持有一个结构，里面有最后一次调用 `poll` 的结果。
struct TaskFuture {
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
    poll: Poll<()>,
}

struct Task {
    // `Mutex` 让 `Task` 实现了 `Sync`。
    // 在任何给定的时刻只有一个线程可以访问 `task_future`。
    // `Mutex` 不需要在这里有正确性。真正的Tokio
    // 在这里没使用锁，但真正的Tokio有非常多行代码，
    // 放在一篇教程里面写不下。
    task_future: Mutex<TaskFuture>,
    executor: mpsc::Sender<Arc<Task>>,
}

impl Task {
    fn schedule(self: &Arc<Self>) {
        self.executor.send(self.clone());
    }
}
```

为了安排任务，`Arc`被克隆并通过管道发送。现在，我们需要把`调度`函数和[`std::task::Waker`](https://doc.rust-lang.org/std/task/struct.Waker.html)挂钩(hook)。标准库提供了一个低等级 API，使用[manual vtable construction](https://doc.rust-lang.org/std/task/struct.RawWakerVTable.html)(手动虚表结构)。这种策略为实现者提供了最大化的灵活性，但是需要一堆 unsafe 的样板代码。我们将使用[`futures`](https://docs.rs/futures/)crate 提供的[`ArcWake`](https://docs.rs/futures/0.3/futures/task/trait.ArcWake.html)工具，而不是直接使用[`RawVakerVTable`](https://doc.rust-lang.org/std/task/struct.RawWakerVTable.html)。这使得我们可以实现一个简单的 trait 就能暴露我们的`Task`结构体作为一个 waker。

添加以下依赖到`Cargo.toml`中来拉取`futures`。

```toml
futures = "0.3"
```

然后实现`futures::task::ArcWake`。

```rust
use futures::task::{self, ArcWake};
use std::sync::Arc;
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.schedule();
    }
}
```

当上面的定时器线程调用`waker.wake()`时，任务被传到管道中。接下来，我们在`MiniTokio::run()`中实现接收和执行任务。

```rust
impl MiniTokio {
    fn run(&self) {
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }

    /// 初始化一个新的 mini-tokio 实例。
    fn new() -> MiniTokio {
        let (sender, scheduled) = mpsc::channel();

        MiniTokio { scheduled, sender }
    }

    /// 在 mini-tokio 实例上生成一个future
    ///
    /// 给定的 future 被包含在 `Task` 中并被传到 `调度` 队列中
    /// 这个 future 将在调用 `run` 时执行
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender);
    }
}

impl TaskFuture {
    fn new(future: impl Future<Output = ()> + Send + 'static) -> TaskFuture {
        TaskFuture {
            future: Box::pin(future),
            poll: Poll::Pending,
        }
    }

    fn poll(&mut self, cx: &mut Context<'_>) {
        // 允许虚假唤醒，即使一个 future 已经返回了 `Ready`。
        // 然而，轮询一个已经返回了 `Ready` 的future是*不*被允许的。
        // 对此，我们需要在调用前检查 future 是否仍处于挂起状态。
        // 如果不这样做可能导致 panic 。
        if self.poll.is_pending() {
            self.poll = self.future.as_mut().poll(cx);
        }
    }
}

impl Task {
    fn poll(self: Arc<Self>) {
        // 从 `Task` 实例中创建waker。
        // 使用上述提到的 `ArcWake` impl。
        let waker = task::waker(self.clone());
        let mut cx = Context::from_waker(&waker);

        // 没有其他线程尝试锁这个 task_future。
        let mut task_future = self.task_future.try_lock().unwrap();

        // 轮询内部的 future
        task_future.poll(&mut cx);
    }

    // 对于给定的 future 生成新任务
    //
    // 初始化包含给定 future 的新任务结构，推给 `sender`
    // 管道的接收部分会获取到这个任务并执行它。
    fn spawn<F>(future: F, sender: &mpsc::Sender<Arc<Task>>)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            task_future: Mutex::new(TaskFuture::new(future)),
            executor: sender.clone(),
        });

        let _ = sender.send(task);
    }
}
```

这里发生了很多事情。首先，实现了`MiniTokio::run()`。该函数在循环体中执行，不断从管道中接收计划执行的任务。让任务被唤醒时，就被推送到管道中，然后这些任务就可以被执行来取得一些进展。

此外，`MiniTokio::new()`和`MiniTokio::spawn()`函数也被调整为使用管道，而不是一个`VecDeque`。当新任务生成时，他们会获取管道发送者的克隆，这让任务可以在运行时上调度自己。

`Task::poll()`函数使用`futures`crate 的[`ArcWake`](https://docs.rs/futures/0.3/futures/task/trait.ArcWake.html)创建了 waker。这个 waker 被用来创建`task::Context`的，它会被传递到`poll`。

# 总结

我们已经看到异步 Rust 如何端对端地工作。Rust 的`async/await`功能是由 trait 提供的。这让第三方 crate，比如 Tokio，提供执行的相关细节。

- 异步 Rust 操作是惰性的，需要调用者轮询它们。
- Waker 被传递给 future，把 future 与调用的任务联系起来。
- 当资源**未**就绪去完成操作，返回`Poll::Pending`并记录任务的 waker。
- 当资源就绪，任务的 waker 会收到通知。
- 当执行器收到通知，就会安排任务执行。
- 再次轮询任务，这次资源就绪，任务可以推进。

# 一些尚未解决的问题

回想当我们实现`Delay`的 future，我们说还有些问题需要解决。Rust 异步模型允许单个 future 在执行时夸任务迁移。考虑以下代码：

```rust
use futures::future::poll_fn;
use std::future::Future;
use std::pin::Pin;

#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let mut delay = Some(Delay { when });

    poll_fn(move |cx| {
        let mut delay = delay.take().unwrap();
        let res = Pin::new(&mut delay).poll(cx);
        assert!(res.is_pending());
        tokio::spawn(async move {
            delay.await;
        });

        Poll::Ready(())
    }).await;
}
```

`pull_fn`函数使用闭包创建一个`Future`实例。上述代码片段创建了一个`Delay`实例，并轮询了一次，然后把`Delay`实例发送给等待它的新任务。此例中，使用**不同的**Waker 实例会导致多次调用`Delay::poll`。当这种情况发生时，你需要保证当 Waker 传递到最近一次轮询调用时调用唤醒。

当实现 future 时，你不能认为每次调用`poll`都**能**提供不同的`Waker`实例。poll 函数必须使用新的 waker 来更新任何先前记录的 waker。

我们稍早前实现的`Delay`每次轮询时都会产生一个新线程。这没啥问题，但如果轮询非常频繁可能导致效率低下（例如，如果你`select!`这个 future 和一些其他 future，只要发生事件就开始轮询二者）。一种方法是记住是否你已经生成了一个线程，尽在你还没生成线程时，才生成一个新线程。但是，如果这样做，你必须保证线程的`Waker`在调用 poll 之后更新，否则你就不会唤醒最新的`Waker`

为了修复之前的实现，我们可以这样做：

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

struct Delay {
    when: Instant,
    // 当我们生成了线程，这里是 Some，否则就是 None。
    waker: Option<Arc<Mutex<Waker>>>,
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // 检查当前实例。如果经过了设定时间，就说明
        // 该 future 完成了，返回 `Poll::Ready`。
        if Instant::now() >= self.when {
            return Poll::Ready(());
        }

        // 设定时间未完成。如果这事第一次调用 future，
        // 生成定时器线程。如果定时器线程已经运行了，
        // 确保存储的 `Waker` 匹配当前任务的waker。
        if let Some(waker) = &self.waker {
            let mut waker = waker.lock().unwrap();

            // 检查存储的 waker 是否匹配当前任务的 waker
            // 这对于 `Delay` 的 future 实例是必须的，因为它可能移动到
            // 一个不同的任务，在两次调用 `poll`之间。如果这发生了
            // waker 包含的给定 `Context` 就会不同，所以我们必须
            // 更新存储的 waker ，来反映这种改变
            if !waker.will_wake(cx.waker()) {
                *waker = cx.waker().clone();
            }
        } else {
            let when = self.when;
            let waker = Arc::new(Mutex::new(cx.waker().clone()));
            self.waker = Some(waker.clone());

            // 第一次调用 `poll`，生成定时器线程。
            thread::spawn(move || {
                let now = Instant::now();

                if now < when {
                    thread::sleep(when - now);
                }

                // 经过了给定时间。通过调用waker来通知调用者
                let waker = waker.lock().unwrap();
                waker.wake_by_ref();
            });
        }

        // 现在，waker已经被存储，计时器线程也已经开启。
        // 还没经过给定的时间（回想一下我们检查过这个事）
        // 因此future还没完成，我们需要返回 `Poll::Pending`
        //
        // `Future` trait 的实现需要当返回 `Pending` 时，
        // future 确保一旦 future 应该被再次轮询时，
        // 给定的 waker 已经收到信号。在我们的例子中，
        // 通过在这里返回 `Pending`，我们承诺一旦经过了给定的时长
        // 我们将调用包含 `Context` 参数的给定的waker。
        // 我们通过上面生成的计时器线程来确保这一点。
        //
        // 如果我们忘记调用 waker，任务就会无限期挂起
        Poll::Pending
    }
}
```

这有点复杂，但是想法是，每次调用`poll`的时候，future 会检查提供的 waker 是否与之前记录的 waker 相匹配。如果两个 waker 匹配，不用执行其他操作。如果不比配，那么记录的 waker 必须被更新。

## 通知工具 Notify utility

我们演示了一个`Delay`future 是如何通过使用 waker 手动实现的。Waker 是异步 Rust 工作的基础。通常情况下，没必要理解到这个层次。例如，在例子`Delay`中，我们可以使用[`tokio::sync::Notify`](https://docs.rs/tokio/1/tokio/sync/struct.Notify.html)工具来为它实现`async/await`。该工具提供了基本的任务通知机制。它会处理 waker 的细节，包含确保记录的 waker 与当前任务相匹配。

使用[`Notify`](https://docs.rs/tokio/1/tokio/sync/struct.Notify.html)，我们可以通过`async/await`来实现`delay`函数，就像这样：

```rust
use tokio::sync::Notify;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

async fn delay(dur: Duration) {
    let when = Instant::now() + dur;
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    thread::spawn(move || {
        let now = Instant::now();

        if now < when {
            thread::sleep(when - now);
        }

        notify_clone.notify_one();
    });


    notify.notified().await;
}
```
