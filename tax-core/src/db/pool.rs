//! Connection pool settings shared by every database backend.

use std::time::Duration;

/// Default for [`PoolConfig::max_connections`].
const DEFAULT_MAX_CONNECTIONS: u32 = 10;
/// Default for [`PoolConfig::min_connections`].
const DEFAULT_MIN_CONNECTIONS: u32 = 0;
/// Default for [`PoolConfig::acquire_timeout`].
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
/// Default for [`PoolConfig::idle_timeout`].
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Default for [`PoolConfig::max_lifetime`].
const DEFAULT_MAX_LIFETIME: Duration = Duration::from_secs(30 * 60);

/// Limits and timeouts for a backend's connection pool.
///
/// The fields mirror the options on SQLx's pool builder without making this
/// crate depend on SQLx. Each backend crate converts the values into its own
/// pool type. The defaults follow SQLx's documented pool defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolConfig {
    /// Most connections the pool keeps open at once. Must be at least 1.
    pub max_connections: u32,
    /// Connections the pool keeps open while idle.
    pub min_connections: u32,
    /// Longest wait for a free connection before the request fails.
    pub acquire_timeout: Duration,
    /// Idle time after which a connection is closed. `None` never closes an
    /// idle connection.
    pub idle_timeout: Option<Duration>,
    /// Age after which a connection is closed and replaced. `None` sets no
    /// age limit.
    pub max_lifetime: Option<Duration>,
    /// Whether a connection is checked before it is handed out.
    pub test_before_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_CONNECTIONS,
            min_connections: DEFAULT_MIN_CONNECTIONS,
            acquire_timeout: DEFAULT_ACQUIRE_TIMEOUT,
            idle_timeout: Some(DEFAULT_IDLE_TIMEOUT),
            max_lifetime: Some(DEFAULT_MAX_LIFETIME),
            test_before_acquire: true,
        }
    }
}

impl PoolConfig {
    /// Returns a copy in which values a pool cannot use are replaced.
    ///
    /// * `max_connections` is raised to at least 1.
    /// * `min_connections` is lowered to `max_connections`.
    /// * A zero `acquire_timeout` becomes the default.
    /// * A zero `idle_timeout` or `max_lifetime` becomes `None`.
    pub fn normalized(&self) -> Self {
        let max_connections = self.max_connections.max(1);
        let acquire_timeout = if self.acquire_timeout.is_zero() {
            DEFAULT_ACQUIRE_TIMEOUT
        } else {
            self.acquire_timeout
        };

        Self {
            max_connections,
            min_connections: self.min_connections.min(max_connections),
            acquire_timeout,
            idle_timeout: self.idle_timeout.filter(|timeout| !timeout.is_zero()),
            max_lifetime: self.max_lifetime.filter(|lifetime| !lifetime.is_zero()),
            test_before_acquire: self.test_before_acquire,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::PoolConfig;

    #[test]
    fn default_values() {
        let pool = PoolConfig::default();

        assert_eq!(pool.max_connections, 10);
        assert_eq!(pool.min_connections, 0);
        assert_eq!(pool.acquire_timeout, Duration::from_secs(30));
        assert_eq!(pool.idle_timeout, Some(Duration::from_secs(600)));
        assert_eq!(pool.max_lifetime, Some(Duration::from_secs(1800)));
        assert!(pool.test_before_acquire);
    }

    #[test]
    fn normalized_leaves_usable_values_unchanged() {
        let pool = PoolConfig::default();

        assert_eq!(pool.normalized(), pool);
    }

    #[test]
    fn normalized_raises_zero_max_connections() {
        let pool = PoolConfig {
            max_connections: 0,
            ..PoolConfig::default()
        };

        assert_eq!(pool.normalized().max_connections, 1);
    }

    #[test]
    fn normalized_caps_min_connections_at_max() {
        let pool = PoolConfig {
            max_connections: 2,
            min_connections: 5,
            ..PoolConfig::default()
        };

        assert_eq!(pool.normalized().min_connections, 2);
    }

    #[test]
    fn normalized_replaces_zero_durations() {
        let pool = PoolConfig {
            acquire_timeout: Duration::ZERO,
            idle_timeout: Some(Duration::ZERO),
            max_lifetime: Some(Duration::ZERO),
            ..PoolConfig::default()
        };
        let normalized = pool.normalized();

        assert_eq!(normalized.acquire_timeout, Duration::from_secs(30));
        assert_eq!(normalized.idle_timeout, None);
        assert_eq!(normalized.max_lifetime, None);
    }
}
