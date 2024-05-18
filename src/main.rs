use mini_redis::{Connection, Frame};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    // 把listener绑定到这个地址
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        // 第二个值包含了新连接的IP和端口
        let (socket, _) = listener.accept().await.unwrap();
        // 每个传入的套接字都会被生成一个新任务。套接字（的所有权）
        // 被移动到新任务并且在那儿处理。
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}

async fn process(socket: TcpStream) {
    use mini_redis::Command::{self, Get, Set};
    use std::collections::HashMap;

    // HashMap用来存储值
    let mut db = HashMap::new();

    // 由 `mini-redis` 提供的 Connection ，处理解析从套接字传过来的帧
    // (handles parsing frames from the socket)
    let mut connection = Connection::new(socket);

    // 使用 `read_frame` 来从连接中接收一条指令。
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                // 值被存储为 `Vec<u8>`
                db.insert(cmd.key().to_string(), cmd.value().to_vec());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    // `Frame::Bulk` 期望数据类型是 `Bytes`。
                    // 这种类型在本教程中稍后介绍。现在，
                    // `&Vec<u8>` 可以使用 `into()` 转换为 `Bytes`。
                    Frame::Bulk(value.clone().into())
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
