# Getting started with Tracing 开始使用 Tracing 日志

[`tracing`](https://docs.rs/tracing)crate 是一个用来检测 Rust 程序收集结构化的，基于事件的诊断信息的框架。

在像 Tokio 这样的异步系统中，解释传统的日志信息可能非常具有挑战性。由于多个任务在同一线程上运行，因此关联的事件和日志行混合在一起，使得跟踪逻辑运行变得困难。`tracing`扩展了日志式诊断，允许库和应用来记录结构化的事件，以及具有*时间性*和*因果关系*的附加信息——与日志消息不同的是，在`tracing`中的一个[`Span`](https://docs.rs/tracing/latest/tracing/#spans)具有开始和结束时间，可以通过执行流进入和退出，并可以存在于相似跨度的潜逃树中。为了表示某一时刻发生的事情，`tracing`提供了事件的补充概念。[`Span`](https://docs.rs/tracing/latest/tracing/#spans)和[`Event`](https://docs.rs/tracing/latest/tracing/#events)都是结构化的，可以记录键入的数据和文本信息。

你可以使用`tracing`来：

- 将分布式跟踪发送到[`OpenTelemetry`](https://docs.rs/tracing-opentelemetry)收集器
- 通过[Tokio 控制台](https://docs.rs/console-subscriber)来调试
- 向[`stdout`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html)，[一个日志文件](https://docs.rs/tracing-appender/latest/tracing_appender/)，或[`journald`](https://docs.rs/tracing-journald/latest/tracing_journald/)输出日志
- [配置](https://docs.rs/tracing-timing/latest/tracing_timing/)你的应用花时间的地方

# 安装

开始，我们需要添加[`tracing`](https://docs.rs/tracing)和[`tracing-subscriber`](https://docs.rs/tracing-subscriber)作为依赖：

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```

`tracing`crate 提供了使用发出追踪的 API。`tracing-subscriber`crate 提供了一些基本的实用程序，用于将这些追踪转发到外部监听器（比如`stdout`）。

# 订阅追踪

如果您正在编写一个可执行文件（而不是库），你需要注册一个追踪[订阅者](https://docs.rs/tracing/latest/tracing/#subscribers)(tracing subscriber)。订阅者是处理应用和依赖发出的跟踪的类型，并可以执行例如计算指标，监视错误以及向外界重新发送跟踪（例如`jurnald`，`stdout`，或`open-telemetry`守护进程）等任务。

大多数情况下，你应该在`main`函数中注册你的跟踪订阅者越早越好。例如，由[`tracing-subscriber`](https://docs.rs/tracing-subscriber)提供的[`FmtSubscriber`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html)类型会打印格式化的跟踪日志和事件到`stdout`，并可以像这样注册：

```rust
#[tokio::main]
pub async fn main() -> mini_redis::Result<()> {
    // 构造一个订阅者，以向 stdout 打印格式化跟踪日志
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    // 在此之后，使用这个订阅者来进行触发日志
    tracing::subscriber::set_global_default(subscriber)?;
    ...
}
```

如果你运行这个程序，你可能会看到一些被 Tokio 触发的跟踪事件，但是你需要修改自己的应用来触发跟踪，以充分利用`tracing`。

## 订阅者设置

在上面的示例中，我们已经使用默认设置设置了`FmtSubscriber`。其实，`tracing-subscriber`也提供了很多方法来设置`FmtSubscriber`，比如自定义输出格式，包括一些额外信息（比如线程 ID 或源代码位置）在日志中，并把日志写到不是`stdout`的其他地方。

例如：

```rust
// 开始设置 `fmt` 订阅者
let subscriber = tracing_subscriber::fmt()
    // 使用更紧凑的日志格式
    .compact()
    // 显示源代码文件路径
    .with_file(true)
    // 显示源代码所在行数
    .with_line_number(true)
    // 显示我们记录事件发生的线程ID
    .with_thread_ids(true)
    // 不要显示事件的目标路径（模块路径）
    .with_target(false)
    // 生成订阅者
    .finish();
```

有关于配置选项的详细信息，可以看[`tracing_subscriber::fmt`的文档](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html#configuration)。

除了`tracing-subscriber`中的`FmtSubscriber`，其他的`Subscriber`也可以实现它们自己记录`tracing`数据的方式。这包含替代输出格式，分析和聚合，以及与其他系统集成，例如分布式跟踪或日志聚合服务。许多 crate 都提供了`Subscriber`的实现。看[这里](https://docs.rs/tracing/latest/tracing/index.html#related-crates)来获取其他`Subscriber`的实现（不完整）列表。

最后，某些情况下，将记录跟踪的多种方式组合到一起，构建实现多种行为的`Subscriber`可能很有用。例如，`tracing-subscriber`crate 提供了[`Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/index.html)trait，可以表示与其他层组合在一起的订阅者组件。看这里了解使用[`Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/index.html)的细节。

# 触发 Span 和事件。

最简单触发 span 的方法是使用`tracing`提供的[`instrument`](https://docs.rs/tracing/latest/tracing/attr.instrument.html)预处理宏。，这可以重写函数体来触发 span，每次调用的时候都会；例如：

```rust
#[tracing::instrument]
fn trace_me(a: u32, b: u32) -> u32 {
    a + b
}
```

每次调用`trace_me`都会触发`tracing`Span：

1. 具有详细的`info`[级别](https://docs.rs/tracing/latest/tracing/struct.Level.html)（“中间立场”的程度）
2. 被命名为`trace_me`
3. 有`a`和`b`的值，是`trace_me`的参数

`instrument`属性是可高度配置的；例如，跟踪[`mini-redis-server`](https://tokio.rs/tokio/tutorial/setup#mini-redis)中处理每个连接的方法：

```rust
use tracing::instrument;

impl Handler {
    /// 处理单个连接
    #[instrument(
        name = "Handler::run",
        skip(self),
        fields(
            // `%` 序列化了对等IP地址通过 `Display` trait
            peer_addr = %self.connection.peer_addr().unwrap()
        ),
    )]
    async fn run(&mut self) -> mini_redis::Result<()> {
        ...
    }
}
```

[`mini-redis-server`](https://tokio.rs/tokio/tutorial/setup#mini-redis)现在会出发`tracing` Span，对于每个传入连接：

1. 具有详细的`info`[级别](https://docs.rs/tracing/latest/tracing/struct.Level.html)（“中间立场”的程度）
2. 被命名为`Hanler::run`
3. 有一些结构化的数据。
   - `fields(...)`指示发出 span*应该*在名为`peer_addr`字段中包含连接的`SocketAddr`的`fmt::Display`表示形式。
   - `skip(self)`指示发出 span*应该不*记录`Hanler`的调试形式。

你还可以通过调用[`span!`](https://docs.rs/tracing/*/tracing/macro.span.html)宏来手动构建[`Span`](https://docs.rs/tracing/latest/tracing/#spans)，或任何其他级别的宏（[`error_span!`](https://docs.rs/tracing/*/tracing/macro.error_span.html), [`warn_span!`](https://docs.rs/tracing/*/tracing/macro.warn_span.html), [`info_span!`](https://docs.rs/tracing/*/tracing/macro.info_span.html), [`debug_span!`](https://docs.rs/tracing/*/tracing/macro.debug_span.html), [`trace_span`](https://docs.rs/tracing/*/tracing/macro.trace_span.html)）。

要触发事件，使用[`event!`](https://docs.rs/tracing/*/tracing/macro.event.html)宏，或者任何其他级别的宏（[`error!`](https://docs.rs/tracing/*/tracing/macro.error.html), [`warn!`](https://docs.rs/tracing/*/tracing/macro.warn.html), [`info!`](https://docs.rs/tracing/*/tracing/macro.info.html), [`debug!`](https://docs.rs/tracing/*/tracing/macro.debug.html), [`trace!`](https://docs.rs/tracing/*/tracing/macro.trace.html)）。例如，记录客户端发送了格式错误的命令的日志：

```rust
// 转换 redis frame 到一个指令结构体。
// 如果 frame 不是一个可用的 redis 指令会返回错误
let cmd = match Command::from_frame(frame) {
    Ok(cmd) => cmd,
    Err(cause) => {
        // frame格式不正确无法解析
        // 这可能表明客户端有问题 (相对服务端来说）
        // 所以我们(1)触发一个warning
        //
        // 这里的宏语法是由 `tracing` crate 提供的
        // 它可被认为类似于
        //      tracing::warn! {
        //          cause = format!("{}", cause),
        //          "failed to parse command from frame"
        //      };
        // `tracing` 提供了结构化的日志,
        // 所以信息是通过key-value对“记录“的
        tracing::warn! {
            %cause,
            "failed to parse command from frame"
        };
        // ...然后 (2) 给客户端发回了error响应
        Command::from_error(cause)
    }
};
```

如果运行应用，你会看到为其处理的每个传入连接触发的 span 上下文装饰的事件。
