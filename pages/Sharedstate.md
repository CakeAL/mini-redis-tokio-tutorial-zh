# Shared state 共享状态

到目前为止，我们已经有了一个可正常工作的 key-value 服务端。然而，有个主要问题：状态不能跨连接共享。我们将在本文中解决。

# 策略

有好几种在 Tokio 中共享状态的方法。

1. 使用互斥体(Mutex)保护(Guard)共享状态。
2. 生成一个任务来管理状态，并使用消息传递(message passing)来操作它。

通常，您应该对简单数据使用第一种方法，对异步任务使用第二种方法（例如 I/O 原语操作）。在本章中，共享的数据是`HashMap`，对应的操作是`insert`和`get`。这两种操作都不是异步的，所以我们使用`Mutex`。

下一章将会介绍后一种方法。

# 添加`bytes`依赖

Mini-Redis 不用`Vec<u8>`，而是使用[bytes](https://docs.rs/bytes/1/bytes/struct.Bytes.html)库中的`Bytes`类型。`Bytes`的目标是为网络编程提供一种健壮的(robust)字节数组结构。它比较`Vec<u8>`添加的最大的特性就是浅克隆(shallow cloning)。换句话说，在`Bytes`实例上调用`clone()`不会导致底层数据被复制。相反的，`Bytes`实例是某些底层数据的引用计数(reference-counted)。`Bytes`大概是`Arc<Vec<u8>>`，但有些额外功能。

添加`bytes`库，需要在`Cargo.toml`中的`[dependencies]`添加：

```toml
bytes = "1"
```

# 初始化`HashMap`

`HashMap`将会在很多任务和潜在的许多线程中共享。为了支持这一点，需要包装在`Arc<Mutex<_>>`中。

首先，为了方便，在`use`语句后面添加一个类型别名。

```rust
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```

然后，更新`main`函数来初始化`HashMap`，并且把`Arc`**句柄(handle)**传递给`process`函数。使用`Arc`允许`Hashmap`在很多任务中被引用，而这些任务可能运行在很多线程上。在整个 Tokio 中，术语句柄(handle)用来指代提供对某些共享状态访问权限的引用值。

```rust
use tokio::net::TcpListener;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    println!("Listening");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // Clone the handle to the hash map.
        let db = db.clone();

        println!("Accepted");
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}
```

# 关于使用`std::sync::Mutex`

注意，使用`std::sync::Mutex`而**不是**`tokio::sync::Mutex`来守卫(guard)`HashMap`。一个常见的错误，是在异步代码中全都用`tokio::sync::Mutex`。异步互斥体(async mutex)是在调用`.await`时锁定(locked)的互斥体。

同步互斥体(sync mutex)会在等待请求锁(lock)时，阻塞当前线程。这样的话，将会阻止其他任务的处理。但是，使用`tokio::sync::Mutex`也没啥用。因为异步互斥体内部使用了同步互斥体。

根据经验，只要数据竞争保持在较低水平并且调用`.await`没有持有锁，就可以在异步代码中使用同步互斥体。

# 更新`process()`

process 函数不再初始化`Hashmap`。相反，它会使用`HashMap`的共享句柄来作为参数。当然在使用之前，也需要先给`HashMap`上锁。记住`HashMap`的 value 类型现在是`Bytes`（可以廉价地克隆），所以这个也得改。

```rust
use tokio::net::TcpStream;
use mini_redis::{Connection, Frame};

async fn process(socket: TcpStream, db: Db) {
    use mini_redis::Command::{self, Get, Set};

    // 由 `mini-redis` 提供的 Connection ，处理解析从套接字传过来的帧
    // (handles parsing frames from the socket)
    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone())
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

# 任务，线程，以及数据竞争

当数据竞争最少时，使用阻塞互斥锁(blocking mutex)来守卫(guard)较小的临界区(short critical sections)是可被接受的。当锁被争用时，执行任务的线程必须阻塞，并等待互斥体解锁。这不仅仅会阻塞当前任务，也同样会阻塞这个线程上调度的其他所有任务。

默认情况下，Tokio 运行时使用多线程调度器。任务会被*运行时*的调度器调度到任意数量的线程上。如果大量的任务都调度执行，并且它们都需要访问同一个互斥体，就会出现数据竞争。另一方面，如果 Tokio 使用[`current_thread`](https://docs.rs/tokio/1/tokio/runtime/index.html#current-thread-scheduler)运行时（当前线程运行时），那么互斥体将永远不会发生争用。

> **info** > [`current_thread`运行时](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_current_thread)是一个轻量化的，单线程的运行时。当仅生成少量任务并且打开少量套接字(socket)时，这是个好选择。例如，当提供一个同步 API 桥(synchronous API bridge)在异步客户端库之上，这个选择运行效果很好。

如果同步互斥锁上的数据竞争成为问题，那么最好的结局方法并不是切换到 Tokio 互斥锁。考虑下面的选择：

- 使用一个专用任务，来管理状态，并使用消息传递。
- 对互斥体分片。
- 重构代码，来避免互斥体。

在我们的例子中，由于每个键都是独立的，所以把互斥体分片(mutex sharding)效果很好。为此，我们将引入`N`个不同的实例，而不是使用单个`Mutex<HashMap<_, _>>`。

```rust
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Vec<u8>>>>>;

fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}
```

然后呢，找到给定的 key 对应的值就变成了两步操作。首先，key 用来识别它属于哪一个分片。然后，在`HashMap`中查找 key。

```rust
let shard = db[hash(key) % db.len()].lock().unwrap();
shard.insert(key, value);
```

上面说的简单实现需要使用固定数量的分片，并且一旦创建分片 map，分片的数量就不能更改了。[`dashmap`](https://docs.rs/dashmap)提供了更复杂的分片哈希图(hash map)的实现。

# 在调用`.await`时持有`MutexGuard`

你可能像这样写代码：

```rust
use std::sync::{Mutex, MutexGuard};

async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    *lock += 1;

    do_something_async().await;
} // 锁在此离开了作用域
```

当你尝试调用此函数时，你会遇到以下错误信息：

```
error: future cannot be sent between threads safely
   --> src/lib.rs:13:5
    |
13  |     tokio::spawn(async move {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    |
   ::: /playground/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-0.2.21/src/task/spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait `std::marker::Send` is not implemented for `std::sync::MutexGuard<'_, i32>`
note: future is not `Send` as this value is used across an await
   --> src/lib.rs:7:5
    |
4   |     let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    |         -------- has type `std::sync::MutexGuard<'_, i32>` which is not `Send`
...
7   |     do_something_async().await;
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^ await occurs here, with `mut lock` maybe used later
8   | }
    | - `mut lock` is later dropped here
```

这是因为`std::sync::MutexGuard`类型**不是**`Send`的。这意味着你不能发送(send)一个互斥锁到另外一个线程，这会报错，原因是 Tokio 运行时可以在任务调用`.await`时，在线程间移动它。为了避免这种情况，你应该重构代码，在调用`.await`之间，互斥锁就析构掉。

```rust
// 这样是正确的！
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    {
        let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
        *lock += 1;
    } // 锁在此离开了作用域

    do_something_async().await;
}
```

注意，这样不行：

```rust
use std::sync::{Mutex, MutexGuard};

// This fails too.
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    *lock += 1;
    drop(lock);

    do_something_async().await;
}
```

这是因为编译器当前只能根据作用域来判断 future 是否是`Send`的。希望编译器之后能更新，来支持显式 drop，但是现在不行，必须使用作用域。

注意，此处讨论的错误在[Spawning 章节的 Send bound 部分](https://tokio.rs/tokio/tutorial/spawning#send-bound)也讨论了。

你不该尝试生成不需要`Send`的任务来规避这个问题，因为如果 Tokio 在`.await`初挂起你的任务，同时这个任务持有锁，一些其他的任务可能被调度到相同的线程上，然后这些其他任务或许也会尝试锁定这个互斥体(lock that mutex)，这可能导致死锁(deadlock)，因为等待锁定互斥体的任务将阻止持有互斥锁的任务释放这个互斥锁(releasing the mutex)。

我们将讨论一些如何修复以下错误信息的方法：

## 重构代码，让它不跨`.await`持有锁

我们已经在上面代码片段中看到了一个例子，但是我们还有更强大的方法可以做到这一点。例如，你可以把互斥锁包装在结构体中，并且仅在该结构体的非异步方法内来给互斥体上锁。

```rust
use std::sync::Mutex;

struct CanIncrement {
    mutex: Mutex<i32>,
}
impl CanIncrement {
    // This function is not marked async.
    fn increment(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock += 1;
    }
}

async fn increment_and_do_stuff(can_incr: &CanIncrement) {
    can_incr.increment();
    do_something_async().await;
}
```

这种模式可以保证你不会遇到`Send`错误，因为互斥锁守卫(mutex guard)没有出现在异步函数中的任何位置。

## 生成一个任务，来管理状态，使用消息传递来操作它

这是本章节开头提到的第二种方法，当在 I/O 资源中共享资源时很常用。参阅下一章节了解更多细节。

## 使用 Tokio 提供的异步互斥体

也可以用 Tokio 提供的`tokio::sync::Mutex`类型。Tokio 互斥锁主要功能就是它可以在调用`.await`时保持，不会出现其他问题。另外提一下，异步互斥体(asynchronous mutex)比普通的互斥体(ordinary mutex)更昂贵（在时间空间上），所以通常最好使用其他两种方法。

```rust
use tokio::sync::Mutex; // 注意！这里使用了 Tokio mutex

// 这可以过编译！
// （但是这种情况重构代码可能更好）
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().await;
    *lock += 1;

    do_something_async().await;
} // 锁在此离开了作用域
```
