use std::time::Duration;

use crate::certification::CertificationMode;

/// Type alias for HTTP header name-value pairs.
pub type HeaderField = (String, String);

/// Configuration for a route, extracted from `#[route]` attribute.
///
/// Carries the certification mode, optional TTL override, and additional
/// headers to include in all responses for this route.
///
/// # Defaults
///
/// - `certification`: [`CertificationMode::response_only()`] — response-only
///   certification with wildcard header inclusion.
/// - `ttl`: `None` — no per-route TTL override (the global config applies).
/// - `headers`: empty — no additional headers.
#[derive(Clone, Debug)]
pub struct RouteConfig {
    /// Certification mode for this route.
    pub certification: CertificationMode,

    /// Optional TTL override for this route's cache.
    ///
    /// When set, takes precedence over the global [`crate::config::CacheConfig`] TTL for
    /// this route's dynamic responses.
    pub ttl: Option<Duration>,

    /// Additional headers to include in all responses for this route.
    pub headers: Vec<HeaderField>,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            certification: CertificationMode::response_only(),
            ttl: None,
            headers: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_route_config_uses_response_only() {
        let config = RouteConfig::default();
        assert!(matches!(
            config.certification,
            CertificationMode::ResponseOnly(_)
        ));
        assert!(config.ttl.is_none());
        assert!(config.headers.is_empty());
    }

    #[test]
    fn route_config_clone_and_debug() {
        let config = RouteConfig {
            certification: CertificationMode::skip(),
            ttl: Some(Duration::from_secs(300)),
            headers: vec![("x-custom".to_string(), "value".to_string())],
        };
        let cloned = config.clone();
        let _debug = format!("{:?}", cloned);
        assert!(matches!(cloned.certification, CertificationMode::Skip));
        assert_eq!(cloned.ttl, Some(Duration::from_secs(300)));
        assert_eq!(cloned.headers.len(), 1);
    }

    #[test]
    fn route_config_with_authenticated_mode() {
        let config = RouteConfig {
            certification: CertificationMode::authenticated(),
            ttl: None,
            headers: vec![],
        };
        assert!(matches!(config.certification, CertificationMode::Full(_)));
    }
}
