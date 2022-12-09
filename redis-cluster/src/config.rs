use std::{fmt, path::PathBuf};

use redis::RedisError;
#[cfg(feature = "serde")]
use serde_1::{Deserialize, Serialize};

use crate::{CreatePoolError, Pool, PoolBuilder, PoolConfig, RedisResult, Runtime};

/// Configuration object.
///
/// # Example (from environment)
///
/// By enabling the `serde` feature you can read the configuration using the
/// [`config`](https://crates.io/crates/config) crate as following:
/// ```env
/// REDIS_CLUSTER__URLS=redis://127.0.0.1:7000,redis://127.0.0.1:7001
/// REDIS_CLUSTER__POOL__MAX_SIZE=16
/// REDIS_CLUSTER__POOL__TIMEOUTS__WAIT__SECS=2
/// REDIS_CLUSTER__POOL__TIMEOUTS__WAIT__NANOS=0
/// ```
/// ```rust
/// # use serde_1 as serde;
/// #
/// #[derive(serde::Deserialize)]
/// # #[serde(crate = "serde_1")]
/// struct Config {
///     redis_cluster: deadpool_redis_cluster::Config,
/// }
///
/// impl Config {
///     pub fn from_env() -> Result<Self, config::ConfigError> {
///         let mut cfg = config::Config::builder()
///            .add_source(
///                config::Environment::default()
///                .separator("__")
///                .try_parsing(true)
///                .list_separator(","),
///            )
///            .build()?;
///            cfg.try_deserialize()
///     }
/// }
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_1::Deserialize, serde_1::Serialize))]
#[cfg_attr(feature = "serde", serde(crate = "serde_1"))]
pub struct Config {
    /// Redis URLs.
    ///
    /// See [Connection Parameters](redis#connection-parameters).
    pub urls: Option<Vec<String>>,

    /// [`redis::ConnectionInfo`] structures.
    pub connections: Option<Vec<ConnectionInfo>>,

    /// Pool configuration.
    pub pool: Option<PoolConfig>,
}

impl Config {
    /// Creates a new [`Pool`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// See [`CreatePoolError`] for details.
    pub fn create_pool(&self, runtime: Option<Runtime>) -> Result<Pool, CreatePoolError> {
        let mut builder = self.builder().map_err(CreatePoolError::Config)?;
        if let Some(runtime) = runtime {
            builder = builder.runtime(runtime);
        }
        builder.build().map_err(CreatePoolError::Build)
    }

    /// Creates a new [`PoolBuilder`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// See [`ConfigError`] for details.
    pub fn builder(&self) -> Result<PoolBuilder, ConfigError> {
        let manager = match (&self.urls, &self.connections) {
            (Some(urls), None) => {
                crate::Manager::new(urls.iter().map(|url| url.as_str()).collect())?
            }
            (None, Some(connections)) => crate::Manager::new(connections.clone())?,
            (None, None) => crate::Manager::new(vec![ConnectionInfo::default()])?,
            (Some(_), Some(_)) => return Err(ConfigError::UrlAndConnectionSpecified),
        };
        let pool_config = self.get_pool_config();
        Ok(Pool::builder(manager).config(pool_config))
    }

    /// Returns [`deadpool::managed::PoolConfig`] which can be used to construct
    /// a [`deadpool::managed::Pool`] instance.
    #[must_use]
    pub fn get_pool_config(&self) -> PoolConfig {
        self.pool.unwrap_or_default()
    }

    /// Creates a new [`Config`] from the given Redis URL (like
    /// `redis://127.0.0.1`).
    #[must_use]
    pub fn from_urls<T: Into<Vec<String>>>(urls: T) -> Config {
        Config {
            urls: Some(urls.into()),
            connections: None,
            pool: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            urls: None,
            connections: Some(vec![ConnectionInfo::default()]),
            pool: None,
        }
    }
}

/// This is a 1:1 copy of the [`redis::ConnectionAddr`] enumeration.
/// This is duplicated here in order to add support for the
/// [`serde::Deserialize`] trait which is required for the [`serde`] support.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(crate = "serde_1"))]
pub enum ConnectionAddr {
    /// Format for this is `(host, port)`.
    Tcp(String, u16),

    /// Format for this is `(host, port)`.
    TcpTls {
        /// Hostname.
        host: String,

        /// Port.
        port: u16,

        /// Disable hostname verification when connecting.
        ///
        /// # Warning
        ///
        /// You should think very carefully before you use this method. If
        /// hostname verification is not used, any valid certificate for any
        /// site will be trusted for use from any other. This introduces a
        /// significant vulnerability to man-in-the-middle attacks.
        insecure: bool,
    },

    /// Format for this is the path to the unix socket.
    Unix(PathBuf),
}

impl Default for ConnectionAddr {
    fn default() -> Self {
        Self::Tcp("127.0.0.1".to_string(), 6379)
    }
}

impl From<ConnectionAddr> for redis::ConnectionAddr {
    fn from(addr: ConnectionAddr) -> Self {
        match addr {
            ConnectionAddr::Tcp(host, port) => Self::Tcp(host, port),
            ConnectionAddr::TcpTls {
                host,
                port,
                insecure,
            } => Self::TcpTls {
                host,
                port,
                insecure,
            },
            ConnectionAddr::Unix(path) => Self::Unix(path),
        }
    }
}

impl From<redis::ConnectionAddr> for ConnectionAddr {
    fn from(addr: redis::ConnectionAddr) -> Self {
        match addr {
            redis::ConnectionAddr::Tcp(host, port) => Self::Tcp(host, port),
            redis::ConnectionAddr::TcpTls {
                host,
                port,
                insecure,
            } => ConnectionAddr::TcpTls {
                host,
                port,
                insecure,
            },
            redis::ConnectionAddr::Unix(path) => Self::Unix(path),
        }
    }
}

/// This is a 1:1 copy of the [`redis::ConnectionInfo`] struct.
/// This is duplicated here in order to add support for the
/// [`serde::Deserialize`] trait which is required for the [`serde`] support.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(crate = "serde_1"))]
pub struct ConnectionInfo {
    /// A connection address for where to connect to.
    pub addr: ConnectionAddr,

    /// A boxed connection address for where to connect to.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub redis: RedisConnectionInfo,
}

impl From<ConnectionInfo> for redis::ConnectionInfo {
    fn from(info: ConnectionInfo) -> Self {
        Self {
            addr: info.addr.into(),
            redis: info.redis.into(),
        }
    }
}

impl From<redis::ConnectionInfo> for ConnectionInfo {
    fn from(info: redis::ConnectionInfo) -> Self {
        Self {
            addr: info.addr.into(),
            redis: info.redis.into(),
        }
    }
}

impl redis::IntoConnectionInfo for ConnectionInfo {
    fn into_connection_info(self) -> RedisResult<redis::ConnectionInfo> {
        Ok(self.into())
    }
}

/// This is a 1:1 copy of the [`redis::RedisConnectionInfo`] struct.
/// This is duplicated here in order to add support for the
/// [`serde::Deserialize`] trait which is required for the [`serde`] support.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(crate = "serde_1"))]
pub struct RedisConnectionInfo {
    /// The database number to use. This is usually `0`.
    pub db: i64,

    /// Optionally a username that should be used for connection.
    pub username: Option<String>,

    /// Optionally a password that should be used for connection.
    pub password: Option<String>,
}

impl From<RedisConnectionInfo> for redis::RedisConnectionInfo {
    fn from(info: RedisConnectionInfo) -> Self {
        Self {
            db: info.db,
            username: info.username,
            password: info.password,
        }
    }
}

impl From<redis::RedisConnectionInfo> for RedisConnectionInfo {
    fn from(info: redis::RedisConnectionInfo) -> Self {
        Self {
            db: info.db,
            username: info.username,
            password: info.password,
        }
    }
}

/// This error is returned if the configuration contains an error
#[derive(Debug)]
pub enum ConfigError {
    /// Both url and connection were specified in the config
    UrlAndConnectionSpecified,
    /// The [`redis`] crate returned an error when parsing the config
    Redis(RedisError),
}

impl From<RedisError> for ConfigError {
    fn from(e: RedisError) -> Self {
        Self::Redis(e)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UrlAndConnectionSpecified => write!(
                f,
                "url and connection must not be specified at the same time."
            ),
            Self::Redis(e) => write!(f, "Redis: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}