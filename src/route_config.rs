use std::time::Duration;

use crate::certification::CertificationMode;

/// Type alias for HTTP header name-value pairs.
pub type HeaderField = (String, String);

/// Per-route configuration extracted from the `#[route(...)]` attribute.
///
/// Carries the certification mode, optional TTL override, and additional
/// headers to include in all responses for this route.
///
/// Most routes do not need an explicit `RouteConfig` — the default
/// (`ResponseOnly` certification, no TTL, no extra headers) is
/// automatically applied when no `#[route]` attribute is present.
///
/// # Defaults
///
/// | Field | Default | Meaning |
/// |-------|---------|---------|
/// | `certification` | [`CertificationMode::response_only()`] | Response-only with wildcard headers |
/// | `ttl` | `None` | Uses the global [`CacheConfig`](crate::config::CacheConfig) TTL |
/// | `headers` | `[]` | No additional headers |
///
/// # Usage with the `#[route]` Macro
///
/// ```rust,ignore
/// use ic_asset_router::route;
///
/// // Skip certification for a health-check endpoint:
/// #[route(certification = "skip")]
/// pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> { /* ... */ }
///
/// // Authenticated endpoint with per-route TTL:
/// #[route(certification = "authenticated", ttl = 60)]
/// pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> { /* ... */ }
/// ```
#[derive(Clone, Debug)]
#[derive(Default)]
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
