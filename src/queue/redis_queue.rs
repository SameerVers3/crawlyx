use std::sync::Arc;
use async_trait::async_trait;
use crate::storage::RedisClient;
use super::{Queue, WorkUnit};

pub struct RedisQueue {
    client: RedisClient,
    key: String,
    notify: Arc<tokio::sync::Notify>,
}

impl RedisQueue {
    pub fn new(client: RedisClient, key: String) -> Self {
        Self {
            client,
            key,
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

#[async_trait]
impl Queue for RedisQueue {
    async fn push(&self, work: WorkUnit) {
        let serialized = serde_json::to_string(&work).expect("failed to serialize WorkUnit");
        let cmd = vec!["RPUSH".to_string(), self.key.clone(), serialized];
        let _ = self.client.run_command(&cmd).await;
        self.notify.notify_one();
    }

    async fn pop(&self) -> WorkUnit {
        let cmd = vec!["LPOP".to_string(), self.key.clone()];
        loop {
            match self.client.run_command(&cmd).await {
                Ok(val) => {
                    if let Some(s) = val.as_str() {
                        if let Ok(work) = serde_json::from_str::<WorkUnit>(s) {
                            return work;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("RedisQueue pop error: {}", e);
                }
            }
            // Wait until notified (local push occurred) or time out after 2 seconds to check remotely
            tokio::select! {
                _ = self.notify.notified() => {}
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {}
            }
        }
    }

    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HttpRedisClient;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn test_redis_queue_push_pop() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            // Intercept RPUSH
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 2048];
                let n = socket.read(&mut buf).await.unwrap();
                let req_str = std::str::from_utf8(&buf[..n]).unwrap();
                assert!(req_str.contains("RPUSH"));
                assert!(req_str.contains("https://test-queue.com"));
                
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n[{\"result\":1}]\n";
                socket.write_all(resp.as_bytes()).await.unwrap();
            }
            // Intercept LPOP
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 2048];
                let n = socket.read(&mut buf).await.unwrap();
                let req_str = std::str::from_utf8(&buf[..n]).unwrap();
                assert!(req_str.contains("LPOP"));
                
                let serialized_work = r#"{"url":"https://test-queue.com","current_depth":1,"target_depth":5,"parent_url":null,"shutdown":false}"#;
                let resp_body = format!("[{{\"result\":{}}}]", serde_json::to_string(serialized_work).unwrap());
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}\n", resp_body.len(), resp_body);
                socket.write_all(resp.as_bytes()).await.unwrap();
            }
        });

        let client = RedisClient::Http(Arc::new(HttpRedisClient::new(format!("http://{}", addr), "t".to_string())));
        let queue = RedisQueue::new(client, "myqueue".to_string());

        let work = WorkUnit::new("https://test-queue.com".to_string(), 1, 5, None);
        queue.push(work).await;

        let popped = queue.pop().await;
        assert_eq!(popped.url, "https://test-queue.com");
        assert_eq!(popped.current_depth, 1);
        assert_eq!(popped.target_depth, 5);

        server_handle.await.unwrap();
    }
}
