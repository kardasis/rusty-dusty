use std::env;

use redis::Commands;

#[derive(Debug)]
pub struct CacheError {
    pub msg: String,
}

fn redis_connection() -> Result<redis::Connection, CacheError> {
    let redis_host = env::var("REDIS_HOST").unwrap_or("127.0.0.1".to_string());
    let redis_port = env::var("REDIS_PORT").unwrap_or("6379".to_string());
    let redis_path = format!("redis://{}:{}", redis_host, redis_port);
    match redis::Client::open(redis_path) {
        Ok(client) => match client.get_connection() {
            Ok(conn) => Ok(conn),
            Err(e) => Err(CacheError { msg: e.to_string() }),
        },
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

#[cfg(test)]
pub fn flushdb() -> Result<String, CacheError> {
    let mut connection = redis_connection()?;
     match redis::cmd("FLUSHDB").query::<String>(&mut connection)  {
        Ok(_) => Ok("flushed".to_string()),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}


pub fn get(key: String) -> Result<String, CacheError> {
    let mut connection = redis_connection()?;
    match connection.get(key) {
        Ok(res) => Ok(res),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

pub fn set(key: String, value: String) -> Result<(), CacheError> {
    let mut connection = redis_connection()?;
    match connection.set(key, value) {
        Ok(x) => Ok(x),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

pub fn zadd_multiple(key: &str, item_pairs: Vec<(&str, u64)>) -> Result<(), CacheError> {
    let mut connection = redis_connection()?;
    match connection.zadd_multiple(key, &item_pairs) {
        Ok(()) => Ok(()),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

pub fn del(key: &str) -> Result<(), CacheError> {
    let mut connection = redis_connection()?;
    match connection.del(key) {
        Ok(res) => Ok(res),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

pub fn zrem(key: &str) -> Result<(), CacheError> {
    let mut connection = redis_connection()?;
    match connection.zrembyscore(key, "-inf", "+inf") {
        Ok(res) => Ok(res),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}

pub fn fullzrange(key: &str) -> Result<Vec<String>, CacheError> {
    let mut connection = redis_connection()?;
    match connection.zrangebyscore(key, "-inf", "+inf") {
        Ok(res) => Ok(res),
        Err(e) => Err(CacheError { msg: e.to_string() }),
    }
}
