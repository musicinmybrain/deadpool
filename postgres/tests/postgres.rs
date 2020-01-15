use std::collections::HashMap;
use std::env;
use std::time::Duration;

use futures::future::join_all;
use tokio_postgres::types::Type;

use deadpool_postgres::{Config, Pool};

fn create_pool() -> Pool {
    let cfg = Config::from_env("PG").unwrap();
    cfg.create_pool(tokio_postgres::NoTls)
}

#[tokio::main]
#[test]
async fn test_basic() {
    let pool = create_pool();
    let client = pool.get().await.unwrap();
    let stmt = client.prepare("SELECT 1 + 2").await.unwrap();
    let rows = client.query(&stmt, &[]).await.unwrap();
    let value: i32 = rows[0].get(0);
    assert_eq!(value, 3);
    assert_eq!(client.statement_cache.size(), 1);
}

#[tokio::main]
#[test]
async fn test_prepare_typed() {
    let pool = create_pool();
    let client = pool.get().await.unwrap();
    let stmt = client
        .prepare_typed("SELECT 1 + $1", &[Type::INT2])
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&42i16]).await.unwrap();
    let value: i32 = rows[0].get(0);
    assert_eq!(value, 43i32);
}

#[tokio::main]
#[test]
async fn test_prepare_typed_error() {
    let pool = create_pool();
    let client = pool.get().await.unwrap();
    let stmt = client
        .prepare_typed("SELECT 1 + $1", &[Type::INT2])
        .await
        .unwrap();
    assert!(client.query(&stmt, &[&42i32]).await.is_err());
}

#[tokio::main]
#[test]
async fn test_transaction_1() {
    let pool = create_pool();
    let mut client = pool.get().await.unwrap();
    {
        let txn = client.transaction().await.unwrap();
        let stmt = txn.prepare("SELECT 1 + 2").await.unwrap();
        let rows = txn.query(&stmt, &[]).await.unwrap();
        let value: i32 = rows[0].get(0);
        txn.commit().await.unwrap();
        assert_eq!(value, 3);
    }
    assert_eq!(client.statement_cache.size(), 1);
}

#[tokio::main]
#[test]
async fn test_transaction_2() {
    let pool = create_pool();
    let mut client = pool.get().await.unwrap();
    let stmt = client.prepare("SELECT 1 + 2").await.unwrap();
    {
        let txn = client.transaction().await.unwrap();
        let rows = txn.query(&stmt, &[]).await.unwrap();
        let value: i32 = rows[0].get(0);
        txn.commit().await.unwrap();
        assert_eq!(value, 3);
    }
    assert_eq!(client.statement_cache.size(), 1);
}

#[tokio::main]
#[test]
async fn test_transaction_pipeline() {
    let pool = create_pool();
    let mut client = pool.get().await.unwrap();
    let stmt = client.prepare("SELECT 1 + $1").await.unwrap();
    let txn = client.transaction().await.unwrap();
    let mut futures = vec![];
    for i in 0..100 {
        let stmt = stmt.clone();
        let txn = &txn;
        futures.push(async move {
            let rows = txn.query(&stmt, &[&i]).await.unwrap();
            let value: i32 = rows[0].get(0);
            value
        });
    }
    let results = join_all(futures).await;
    for i in 0..100 {
        assert_eq!(results[i], (i as i32) + 1);
    }
}

struct Env {
    backup: HashMap<String, Option<String>>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            backup: HashMap::new()
        }
    }
    pub fn set(&mut self, name: &str, value: &str) {
        self.backup.insert(name.to_string(), env::var(name).ok());
        env::set_var(name, value);
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (name, value) in self.backup.iter() {
            println!("setting {} = {:?}", name, value);
            match value {
                Some(value) => env::set_var(name.as_str(), value),
                None => env::remove_var(name.as_str()),
            }
        }
    }
}

#[test]
#[cfg(feature = "config")]
fn test_config_from_env() {
    // This test must not use "PG" as prefix as this can cause the other
    // tests which also use the "PG" prefix to fail.
    let mut env = Env::new();
    env.set("PG_ENV_TEST_HOST", "pg.example.com");
    env.set("PG_ENV_TEST_USER", "john_doe");
    env.set("PG_ENV_TEST_PASSWORD", "topsecret");
    env.set("PG_ENV_TEST_DBNAME", "example");
    env.set("PG_ENV_TEST_POOL.MAX_SIZE", "42");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.WAIT.SECS", "1");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.WAIT.NANOS", "0");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.CREATE.SECS", "2");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.CREATE.NANOS", "0");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.RECYCLE.SECS", "3");
    env.set("PG_ENV_TEST_POOL.TIMEOUTS.RECYCLE.NANOS", "0");
    let cfg = Config::from_env("PG_ENV_TEST").unwrap();
    // FIXME add tests for db connection parameters
    //let pg_cfg = cfg.get_pg_config();
    //assert_eq!(pg_cfg.host, vec![tokio_postgres::config::Host::Tcp("pg.example.com")]);
    let pool_cfg = cfg.get_pool_config();
    assert_eq!(pool_cfg.max_size, 42);
    assert_eq!(pool_cfg.timeouts.wait, Some(Duration::from_secs(1)));
    assert_eq!(pool_cfg.timeouts.create, Some(Duration::from_secs(2)));
    assert_eq!(pool_cfg.timeouts.recycle, Some(Duration::from_secs(3)));
}
