use bytes::Bytes;
use mini_redis::{Connection, Frame};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() {
    // 把listener绑定到这个地址
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    println!("Listening");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        // 第二个值包含了新连接的IP和端口
        let (socket, _) = listener.accept().await.unwrap();
        // Clone the handle to the hash map
        // 克隆hash map的句柄
        let db = db.clone();

        println!("Accepted");
        // 每个传入的套接字都会被生成一个新任务。套接字（的所有权）
        // 被移动到新任务并且在那儿处理。
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}

async fn process(socket: TcpStream, db: Db) {
    use mini_redis::Command::{self, Get, Set};

    // 由 `mini-redis` 提供的 Connection ，处理解析从套接字传过来的帧
    // (handles parsing frames from the socket)
    let mut connection = Connection::new(socket);

    // 使用 `read_frame` 来从连接中接收一条指令。
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
