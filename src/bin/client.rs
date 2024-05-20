use bytes::Bytes;
use mini_redis::client;
use tokio::sync::{mpsc, oneshot};

/// 多个不同的命令在单个管道上复用
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}

/// 由请求者提供，并由管理任务来发送指令的响应，返回给请求者
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[tokio::main]
async fn main() {
    // 创建一个容量为32的新管道
    let (tx, mut rx) = mpsc::channel(32);
    // `Sender` 句柄被移动到任务里. 因为这儿有俩任务，
    // 我们需要另一个 `Sender`
    let tx2 = tx.clone();

    // 生成俩任务，一个get一个key，一个set一个key
    let t1 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "foo".to_string(),
            resp: resp_tx,
        };

        // 发送GET请求
        tx.send(cmd).await.unwrap();

        // 等待响应
        let res = resp_rx.await;
        println!("GOT = {:?}", res);
    });

    let t2 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "foo".to_string(),
            val: "bar".into(),
            resp: resp_tx,
        };

        // 发送GET请求
        tx2.send(cmd).await.unwrap();

        // 等待响应
        let res = resp_rx.await;
        println!("GOT = {:?}", res);
    });

    // `move` 关键字用来 **移动** `rx` 的所有权到任务中。
    let manager = tokio::spawn(async move {
        // 与服务器建立连接
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // 开始接收消息
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, resp } => {
                    let res = client.get(&key).await;
                    // 使用 `_` 忽略错误
                    let _ = resp.send(res);
                }
                Command::Set { key, val, resp } => {
                    let res = client.set(&key, val).await;
                    // 使用 `_` 忽略错误
                    let _ = resp.send(res);
                }
            }
        }
    });

    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}
