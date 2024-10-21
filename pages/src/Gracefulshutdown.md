# Graceful shutdown 如何优雅地结束程序

这一页的目的是概述如何在异步应用中正确地关闭程序。

实现优雅地结束程序分为三部分：

- 搞明白何时关闭
- 告知程序每一部分程序关闭
- 等待应用其他部分关闭

本文其余部分将介绍这些。此处描述的方法实现可以在[mini-redis](https://github.com/tokio-rs/mini-redis/)中找到，尤其是[`src/server.rs`](https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs)和[`src/shutdown.rs`](https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs)文件中。

## 搞清何时关闭

这取决于应用程序，当应用接收到从操作系统的信号是一种常见的关闭情况。这种情况会发生，比如当你程序运行时在终端中按下 ctrl+c 时。为了检测这种，Tokio 提供了一个[`tokio::signal::ctrl_c`](https://docs.rs/tokio/1/tokio/signal/fn.ctrl_c.html)函数，该函数会休眠，直到收到这样的信号。你可以这样使用它：

```rust
use tokio::signal;

#[tokio::main]
async fn main() {
    // ... 在单独的任务上生成应用 ...

    match signal::ctrl_c().await {
        Ok(()) => {},
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // 当发生 error 我们也结束程序
        },
    }

    // 向应用发送关机信号，并等待
}
```

如果你有多钟关闭条件，你可以使用[mpsc 管道](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html)来发送关机信号到某个地方。你可以在[`ctrl_c`](https://docs.rs/tokio/1/tokio/signal/fn.ctrl_c.html)和管道之间进行[选择](https://docs.rs/tokio/1/tokio/macro.select.html)，例如：

```rust
use tokio::signal;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();

    // ... 在单独的任务上生成应用 ...
    //
    // 只要从程序内部发出了关闭信号，应用使用 shutdown_send

    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = shutdown_recv.recv() => {},
    }

    // 向应用发送关机信号，并等待
}
```

## 告知程序每一部分程序关闭

当你想要告知更多任务来关闭，你可以使用[Cancellation Tokens](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)。这些 token 允许你来通知任务，它们需要终止它们自己来响应这个取消请求，从而轻松实现正常关闭。

为了在数个任务之间共享`CancellationToken`，你需要克隆它。这是由于单一的所有权规则要求每一个值只能有一个所有者。当克隆一个 token，你会得到一个与原来 token 一样的 token；如果其中一个取消了，那么其他的也会取消。你可以克隆你需要那么多数量的 token，并当你在其中一个 token 上调用`cnacel`，它们全部都会被取消掉。

这里是在多个任务中使用`CancellationToken`的步骤：

1. 首先，创建新的`CancellationToken`。
2. 然后，通过`clone`方法创建`CancellationToken`的克隆。这会创建新的 token 并可以用于其他任务上。
3. 传递原始或者克隆的 token 到应该响应取消请求的任务上。
4. 当你想要优雅地关闭任务时，在原始或者克隆的 token 上调用`cancel`方法。任何任务侦测到从原始或克隆的 token 上的取消请求，将会被通知关闭。

这里是上述方法步骤的代码片段：

```rust
// Step 1: Create a new CancellationToken
let token = CancellationToken::new();

// Step 2: Clone the token for use in another task
let cloned_token = token.clone();

// Task 1 - Wait for token cancellation or a long time
let task1_handle = tokio::spawn(async move {
    tokio::select! {
        // Step 3: Using cloned token to listen to cancellation requests
        _ = cloned_token.cancelled() => {
            // The token was cancelled, task can shut down
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(9999)) => {
            // Long work has completed
        }
    }
});

// Task 2 - Cancel the original token after a small delay
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Step 4: Cancel the original or cloned token to notify other tasks about shutting down gracefully
    token.cancel();
});

// Wait for tasks to complete
task1_handle.await.unwrap()
```

当使用 Cancellation Token，你不必在 token 取消时立刻去关闭任务。相反，您可以在终止任务之前运行关机流程，比如刷新数据到一个文件或者数据库中，或发送一个关闭消息到一个连接中。

## 等待应用其他部分关闭

一旦您告知任务关闭，你需要等待它们完成关机流程。一个简单的方法是使用[任务追踪(task tracker)](https://docs.rs/tokio-util/latest/tokio_util/task/task_tracker)。一个任务追踪器是任务的集合。任务追踪器的[`wait`](https://docs.rs/tokio-util/latest/tokio_util/task/task_tracker/struct.TaskTracker.html#method.wait)方法提供了一个 future，只有在所有任务的 future 都已经解析，**并**任务追踪器已经关闭后，才会进行解析。

下面的示例会生成 10 个任务，然后使用任务追踪器来等待它们关机。

```rust
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::task::TaskTracker;

#[tokio::main]
async fn main() {
    let tracker = TaskTracker::new();

    for i in 0..10 {
        tracker.spawn(some_operation(i));
    }

    // 一旦我们已经生成了所有任务，我们关闭追踪器。
    tracker.close();

    // 等待所有任务完成。
    tracker.wait().await;

    println!("This is printed after all of the tasks.");
}

async fn some_operation(i: u64) {
    sleep(Duration::from_millis(100 * i)).await;
    println!("Task {} shutting down.", i);
}
```
