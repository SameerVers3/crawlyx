use std::sync::Arc;
use serde_json::Value;

#[derive(Clone)]
pub enum RedisClient {
    Http(Arc<HttpRedisClient>),
    Tcp(Arc<TcpRedisClient>),
}

impl RedisClient {
    pub async fn run_command(&self, cmd: &[String]) -> Result<Value, String> {
        match self {
            RedisClient::Http(c) => c.run_command(cmd).await,
            RedisClient::Tcp(c) => c.run_command(cmd).await,
        }
    }
}

pub struct HttpRedisClient {
    client: reqwest::Client,
    url: String,
    token: String,
}

impl HttpRedisClient {
    pub fn new(url: String, token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            token,
        }
    }

    pub async fn run_command(&self, cmd: &[String]) -> Result<Value, String> {
        let endpoint = format!("{}/pipeline", self.url.trim_end_matches('/'));
        let body = serde_json::to_value(vec![cmd]).map_err(|e| e.to_string())?;

        let response = self.client.post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP error: {}", status));
        }

        let mut results: Vec<Value> = response.json().await.map_err(|e| e.to_string())?;
        if results.is_empty() {
            return Err("Empty response from Redis REST API".to_string());
        }

        let mut result_obj = results.remove(0);
        if let Some(err) = result_obj.get("error") {
            return Err(err.as_str().unwrap_or("Unknown Redis error").to_string());
        }

        Ok(result_obj.get_mut("result").cloned().unwrap_or(Value::Null))
    }
}

pub struct TcpRedisClient {
    client: redis::Client,
}

impl TcpRedisClient {
    pub fn new(url: String) -> Result<Self, String> {
        let client = redis::Client::open(url).map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    pub async fn run_command(&self, cmd: &[String]) -> Result<Value, String> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await.map_err(|e| e.to_string())?;

        if cmd.is_empty() {
            return Err("Empty command".to_string());
        }

        let mut redis_cmd = redis::Cmd::new();
        redis_cmd.arg(&cmd[0]);
        for arg in &cmd[1..] {
            redis_cmd.arg(arg);
        }

        let val: redis::Value = redis_cmd.query_async(&mut conn).await.map_err(|e| e.to_string())?;
        Ok(redis_value_to_json(val))
    }
}

fn redis_value_to_json(val: redis::Value) -> Value {
    match val {
        redis::Value::Nil => Value::Null,
        redis::Value::Int(i) => Value::Number(i.into()),
        redis::Value::BulkString(bytes) => {
            if let Ok(s) = std::str::from_utf8(&bytes) {
                Value::String(s.to_string())
            } else {
                Value::String(bytes.iter().map(|b| format!("{:02x}", b)).collect())
            }
        }
        redis::Value::Array(values) => {
            Value::Array(values.into_iter().map(redis_value_to_json).collect())
        }
        redis::Value::SimpleString(s) => Value::String(s),
        redis::Value::Okay => Value::String("OK".to_string()),
        redis::Value::Map(kvs) => {
            let mut map = serde_json::Map::new();
            let mut is_obj = true;
            for (k, v) in &kvs {
                if let redis::Value::SimpleString(s) = k {
                    map.insert(s.clone(), redis_value_to_json(v.clone()));
                } else if let redis::Value::BulkString(bytes) = k {
                    if let Ok(s) = std::str::from_utf8(bytes) {
                        map.insert(s.to_string(), redis_value_to_json(v.clone()));
                    } else {
                        is_obj = false;
                        break;
                    }
                } else {
                    is_obj = false;
                    break;
                }
            }
            if is_obj {
                Value::Object(map)
            } else {
                Value::Array(kvs.into_iter().map(|(k, v)| {
                    Value::Array(vec![redis_value_to_json(k), redis_value_to_json(v)])
                }).collect())
            }
        }
        redis::Value::Boolean(b) => Value::Bool(b),
        redis::Value::Double(f) => {
            if let Some(n) = serde_json::Number::from_f64(f) {
                Value::Number(n)
            } else {
                Value::Null
            }
        }
        _ => Value::Null,
    }
}

pub fn create_redis_client() -> Option<RedisClient> {
    if let (Ok(url), Ok(token)) = (std::env::var("UPSTASH_REDIS_REST_URL"), std::env::var("UPSTASH_REDIS_REST_TOKEN")) {
        Some(RedisClient::Http(Arc::new(HttpRedisClient::new(url, token))))
    } else if let Ok(url) = std::env::var("REDIS_URL") {
        match TcpRedisClient::new(url) {
            Ok(client) => Some(RedisClient::Tcp(Arc::new(client))),
            Err(e) => {
                eprintln!("Failed to initialize Redis TCP client: {}", e);
                None
            }
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn test_http_redis_client_sends_correct_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let server_handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 2048];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = std::str::from_utf8(&buf[..n]).unwrap();
            
            assert!(req_str.to_lowercase().contains("authorization: bearer mock-token"));
            assert!(req_str.contains(r#"[["SADD","visited","https://foo.com"]]"#));
            
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\n\r\n[{\"result\":1}]\n";
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let client = RedisClient::Http(Arc::new(HttpRedisClient::new(format!("http://{}", addr), "mock-token".to_string())));
        let cmd = vec!["SADD".to_string(), "visited".to_string(), "https://foo.com".to_string()];
        let res = client.run_command(&cmd).await.unwrap();
        
        assert_eq!(res.as_i64(), Some(1));
        server_handle.await.unwrap();
    }
}
