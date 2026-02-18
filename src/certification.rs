//! Certification mode configuration for HTTP responses.
//!
//! IC HTTP certification has three fundamental modes that determine which
//! parts of the HTTP request/response are hashed and cryptographically
//! certified. These types let you configure certification granularity
//! per-asset or per-route.
//!
//! # Modes
//!
//! - [`CertificationMode::Skip`] — No certification. Fastest, use for public
//!   endpoints where tampering risk is acceptable.
//! - [`CertificationMode::ResponseOnly`] — Only the response is certified.
//!   Good for static assets where the response depends only on the URL path.
//! - [`CertificationMode::Full`] — Both request and response are certified.
//!   Required when the response depends on request headers (e.g.,
//!   `Authorization`, `Accept`).
//!
//! # Choosing a Mode
//!
//! **Response-only (default)** is correct for 90% of routes — use it when
//! the response depends only on the URL path and the canister state.
//!
//! **Skip** is appropriate for health-check or status endpoints where
//! tampering has no security impact and maximum performance is desired.
//!
//! **Full** (or the [`CertificationMode::authenticated`] preset) is
//! required when the response depends on *who* is making the request
//! (e.g., the `Authorization` header). Without full certification a
//! malicious replica could serve one user's response to another.

/// Certification mode for HTTP responses.
///
/// Determines which parts of the HTTP request/response are hashed and
/// certified by the Internet Computer's boundary nodes. The default mode
/// is [`CertificationMode::ResponseOnly`] with wildcard header inclusion
/// and standard exclusions.
///
/// # When to Use Each Variant
///
/// | Variant | Use when | Example |
/// |---------|----------|---------|
/// | [`Skip`](Self::Skip) | Tampering has no security impact | Health checks, `/ping` |
/// | [`ResponseOnly`](Self::ResponseOnly) | Same URL always returns same content | Static pages, blog posts |
/// | [`Full`](Self::Full) | Response depends on request identity | Authenticated APIs |
///
/// # Examples
///
/// ```
/// use ic_asset_router::CertificationMode;
///
/// // Default: response-only (recommended for most routes)
/// let mode = CertificationMode::default();
/// assert!(matches!(mode, CertificationMode::ResponseOnly(_)));
///
/// // Skip: no certification overhead
/// let mode = CertificationMode::skip();
/// assert!(matches!(mode, CertificationMode::Skip));
///
/// // Authenticated: full certification with Authorization header
/// let mode = CertificationMode::authenticated();
/// assert!(matches!(mode, CertificationMode::Full(_)));
/// ```
#[derive(Clone, Debug)]
pub enum CertificationMode {
    /// No certification. The response is served without cryptographic
    /// verification.
    ///
    /// **Handler execution:** Unlike ResponseOnly and Full modes, skip-mode
    /// routes run the handler on every query call. This makes them behave
    /// like candid `query` calls — fast (~200ms) and executed on a single
    /// replica without consensus. This enables handler-level auth checks
    /// (e.g. validating a JWT or checking `ic_cdk::caller()`) on every
    /// request, which is useful for authenticated API endpoints where
    /// per-call latency matters more than response certification.
    ///
    /// **Security model:** Skip certification provides the same trust level
    /// as candid query calls — both trust the responding replica. The
    /// response is not cryptographically verified by the boundary node in
    /// either case. If candid queries are acceptable for your application,
    /// skip certification is equally acceptable.
    ///
    /// **When to use:**
    /// - Health checks, `/ping`, and other low-value endpoints
    /// - Auth-gated API endpoints where you need fast query-path performance
    ///   with per-call authentication (combine with handler-level auth)
    ///
    /// **When NOT to use:**
    /// - Endpoints where you need the boundary node to cryptographically
    ///   verify the response (use ResponseOnly or Full instead)
    Skip,

    /// Only the response is certified. Request details (headers, query
    /// params) are not included in the certification hash. This is the
    /// **default mode** and is correct for the vast majority of routes
    /// where the response depends only on the URL path and canister state.
    ///
    /// Use [`ResponseOnlyConfig`] to control which response headers
    /// participate in the hash. The response body and status code are
    /// always certified regardless of header configuration.
    ResponseOnly(ResponseOnlyConfig),

    /// Both request and response are certified. Required when the response
    /// depends on request identity — for example, when different
    /// `Authorization` headers produce different responses. Without full
    /// certification a malicious replica could serve one user's cached
    /// response to another user.
    ///
    /// Use [`FullConfig`] (or the [`FullConfigBuilder`]) to specify which
    /// request headers and query parameters participate in the hash.
    /// The request method and body are **always** certified automatically.
    Full(FullConfig),
}

impl CertificationMode {
    /// Create a skip-certification mode.
    ///
    /// Skip-mode routes run the handler on every query call (like candid
    /// queries) and attach a skip certification witness. See
    /// [`CertificationMode::Skip`] for the full security model.
    ///
    /// Equivalent to `CertificationMode::Skip`. Provided for symmetry
    /// with [`response_only()`](Self::response_only) and
    /// [`authenticated()`](Self::authenticated).
    pub fn skip() -> Self {
        Self::Skip
    }

    /// Create a response-only certification mode with the default
    /// [`ResponseOnlyConfig`] (wildcard header inclusion, standard
    /// exclusions).
    ///
    /// This is also what [`CertificationMode::default()`] returns.
    pub fn response_only() -> Self {
        Self::ResponseOnly(ResponseOnlyConfig::default())
    }

    /// Create a full-certification preset for authenticated APIs.
    ///
    /// Includes the `Authorization` request header and `Content-Type`
    /// response header in the certification hash. Use this when the
    /// response depends on the caller's identity — different
    /// `Authorization` tokens will produce independently certified
    /// responses, preventing cross-user response mixing.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_asset_router::CertificationMode;
    ///
    /// let mode = CertificationMode::authenticated();
    /// match mode {
    ///     CertificationMode::Full(config) => {
    ///         assert_eq!(config.request_headers, vec!["authorization"]);
    ///         assert_eq!(config.response.include_headers, vec!["content-type"]);
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn authenticated() -> Self {
        Self::Full(
            FullConfig::builder()
                .with_request_headers(&["authorization"])
                .with_response_headers(&["content-type"])
                .build(),
        )
    }
}

impl Default for CertificationMode {
    fn default() -> Self {
        Self::response_only()
    }
}

/// Configuration for response-only certification.
///
/// Controls which response headers participate in the certification hash.
/// The response body and status code are **always** certified regardless
/// of header configuration.
///
/// # Header Selection
///
/// There are two strategies for selecting headers:
///
/// 1. **Wildcard with exclusions** (default) — set `include_headers` to
///    `["*"]` and list headers to skip in `exclude_headers`. This is the
///    safest default because new headers are automatically covered.
///
/// 2. **Explicit inclusion** — list only the headers you want certified.
///    Use this when you need precise control or want to minimize the
///    certification payload.
///
/// # Default
///
/// The default configuration includes all headers (`"*"`) and excludes
/// `date`, `ic-certificate`, and `ic-certificate-expression` (which
/// are either non-deterministic or managed by the certification layer
/// itself).
#[derive(Clone, Debug)]
pub struct ResponseOnlyConfig {
    /// Response headers to include in the certification hash.
    ///
    /// Set to `vec!["*".to_string()]` to include all headers (with
    /// exclusions applied via [`exclude_headers`](Self::exclude_headers)).
    /// Alternatively, list specific header names for explicit inclusion.
    pub include_headers: Vec<String>,

    /// Response headers to explicitly exclude from certification.
    ///
    /// Applied after `include_headers`. Only meaningful when
    /// `include_headers` contains `"*"`.
    pub exclude_headers: Vec<String>,
}

impl Default for ResponseOnlyConfig {
    fn default() -> Self {
        Self {
            include_headers: vec!["*".to_string()],
            exclude_headers: vec![
                "date".to_string(),
                "ic-certificate".to_string(),
                "ic-certificate-expression".to_string(),
            ],
        }
    }
}

/// Configuration for full request+response certification.
///
/// In full mode the request method and body are **always** certified by
/// `ic-http-certification` — there is no opt-out. The configurable parts
/// are which request headers and query parameters participate in the
/// certification hash.
///
/// # When to Use
///
/// Use full certification when the response depends on details of the
/// incoming request beyond the URL path:
///
/// - **`Authorization` header** — different users receive different
///   responses. Use the [`CertificationMode::authenticated`] preset.
/// - **`Accept` header** — content negotiation (JSON vs HTML).
/// - **Query parameters** — pagination (`?page=2`), filtering, sorting.
///
/// # Which Headers to Certify
///
/// Only certify headers that **affect the response content**. Certifying
/// headers like `User-Agent` causes cache fragmentation (every browser
/// version gets a separate certificate) with no security benefit.
///
/// # Example
///
/// ```
/// use ic_asset_router::FullConfig;
///
/// let config = FullConfig::builder()
///     .with_request_headers(&["authorization", "accept"])
///     .with_query_params(&["page", "limit"])
///     .with_response_headers(&["content-type"])
///     .build();
///
/// assert_eq!(config.request_headers, vec!["authorization", "accept"]);
/// assert_eq!(config.query_params, vec!["page", "limit"]);
/// ```
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct FullConfig {
    /// Request headers to include in the certification hash.
    ///
    /// Only these headers are hashed; all other request headers are
    /// ignored during certification. Header names should be lowercase.
    pub request_headers: Vec<String>,

    /// Query parameters to include in the certification hash.
    ///
    /// When set, requests with different values for these parameters
    /// produce independently certified responses. A malicious replica
    /// cannot serve the `?page=1` response when `?page=2` is requested.
    pub query_params: Vec<String>,

    /// Response certification configuration (header inclusion/exclusion).
    pub response: ResponseOnlyConfig,
}


impl FullConfig {
    /// Create a builder for ergonomic construction of [`FullConfig`].
    pub fn builder() -> FullConfigBuilder {
        FullConfigBuilder::default()
    }
}

/// Builder for [`FullConfig`] with ergonomic chained construction.
///
/// All `with_*_headers` methods normalize header names to lowercase,
/// so `"Authorization"` and `"authorization"` are treated identically.
///
/// # Example
///
/// ```
/// use ic_asset_router::FullConfig;
///
/// let config = FullConfig::builder()
///     .with_request_headers(&["Authorization", "Accept"])
///     .with_query_params(&["page", "limit"])
///     .with_response_headers(&["Content-Type"])
///     .excluding_response_headers(&["Set-Cookie"])
///     .build();
///
/// assert_eq!(config.request_headers, vec!["authorization", "accept"]);
/// assert_eq!(config.query_params, vec!["page", "limit"]);
/// assert_eq!(config.response.include_headers, vec!["content-type"]);
/// assert_eq!(config.response.exclude_headers, vec!["set-cookie"]);
/// ```
#[derive(Default)]
pub struct FullConfigBuilder {
    request_headers: Vec<String>,
    query_params: Vec<String>,
    include_response_headers: Vec<String>,
    exclude_response_headers: Vec<String>,
}

impl FullConfigBuilder {
    /// Set the request headers to include in certification.
    ///
    /// Header names are normalized to lowercase.
    pub fn with_request_headers(mut self, headers: &[&str]) -> Self {
        self.request_headers = headers.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Set the query parameters to include in certification.
    pub fn with_query_params(mut self, params: &[&str]) -> Self {
        self.query_params = params.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set the response headers to include in certification.
    ///
    /// Header names are normalized to lowercase. When set, replaces the
    /// default wildcard inclusion.
    pub fn with_response_headers(mut self, headers: &[&str]) -> Self {
        self.include_response_headers = headers.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Set the response headers to exclude from certification.
    ///
    /// Header names are normalized to lowercase.
    pub fn excluding_response_headers(mut self, headers: &[&str]) -> Self {
        self.exclude_response_headers = headers.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Consume the builder and produce a [`FullConfig`].
    pub fn build(self) -> FullConfig {
        FullConfig {
            request_headers: self.request_headers,
            query_params: self.query_params,
            response: ResponseOnlyConfig {
                include_headers: if self.include_response_headers.is_empty() {
                    vec!["*".to_string()]
                } else {
                    self.include_response_headers
                },
                exclude_headers: self.exclude_response_headers,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_response_only() {
        let mode = CertificationMode::default();
        assert!(matches!(mode, CertificationMode::ResponseOnly(_)));
    }

    #[test]
    fn skip_produces_skip() {
        let mode = CertificationMode::skip();
        assert!(matches!(mode, CertificationMode::Skip));
    }

    #[test]
    fn response_only_has_correct_default_config() {
        let mode = CertificationMode::response_only();
        match mode {
            CertificationMode::ResponseOnly(config) => {
                assert_eq!(config.include_headers, vec!["*"]);
                assert_eq!(
                    config.exclude_headers,
                    vec!["date", "ic-certificate", "ic-certificate-expression"]
                );
            }
            _ => panic!("expected ResponseOnly"),
        }
    }

    #[test]
    fn authenticated_has_authorization_in_request_headers() {
        let mode = CertificationMode::authenticated();
        match mode {
            CertificationMode::Full(config) => {
                assert_eq!(config.request_headers, vec!["authorization"]);
                assert_eq!(config.response.include_headers, vec!["content-type"]);
            }
            _ => panic!("expected Full"),
        }
    }

    #[test]
    fn builder_with_all_options() {
        let config = FullConfig::builder()
            .with_request_headers(&["Authorization", "Accept"])
            .with_query_params(&["page", "limit"])
            .with_response_headers(&["Content-Type", "ETag"])
            .excluding_response_headers(&["Set-Cookie"])
            .build();

        assert_eq!(config.request_headers, vec!["authorization", "accept"]);
        assert_eq!(config.query_params, vec!["page", "limit"]);
        assert_eq!(
            config.response.include_headers,
            vec!["content-type", "etag"]
        );
        assert_eq!(config.response.exclude_headers, vec!["set-cookie"]);
    }

    #[test]
    fn builder_with_partial_options() {
        let config = FullConfig::builder()
            .with_request_headers(&["authorization"])
            .build();

        assert_eq!(config.request_headers, vec!["authorization"]);
        assert!(config.query_params.is_empty());
        // No explicit response headers → wildcard
        assert_eq!(config.response.include_headers, vec!["*"]);
        assert!(config.response.exclude_headers.is_empty());
    }

    #[test]
    fn builder_with_no_options() {
        let config = FullConfig::builder().build();

        assert!(config.request_headers.is_empty());
        assert!(config.query_params.is_empty());
        assert_eq!(config.response.include_headers, vec!["*"]);
        assert!(config.response.exclude_headers.is_empty());
    }

    #[test]
    fn header_normalization_to_lowercase() {
        let config = FullConfig::builder()
            .with_request_headers(&["AUTHORIZATION", "Accept-Encoding"])
            .with_response_headers(&["Content-Type", "X-CUSTOM-HEADER"])
            .excluding_response_headers(&["SET-COOKIE"])
            .build();

        assert_eq!(
            config.request_headers,
            vec!["authorization", "accept-encoding"]
        );
        assert_eq!(
            config.response.include_headers,
            vec!["content-type", "x-custom-header"]
        );
        assert_eq!(config.response.exclude_headers, vec!["set-cookie"]);
    }

    #[test]
    fn full_config_default() {
        let config = FullConfig::default();
        assert!(config.request_headers.is_empty());
        assert!(config.query_params.is_empty());
        assert_eq!(config.response.include_headers, vec!["*"]);
        assert_eq!(
            config.response.exclude_headers,
            vec!["date", "ic-certificate", "ic-certificate-expression"]
        );
    }

    #[test]
    fn response_only_config_default() {
        let config = ResponseOnlyConfig::default();
        assert_eq!(config.include_headers, vec!["*"]);
        assert_eq!(
            config.exclude_headers,
            vec!["date", "ic-certificate", "ic-certificate-expression"]
        );
    }

    #[test]
    fn certification_mode_clone_and_debug() {
        let mode = CertificationMode::authenticated();
        let cloned = mode.clone();
        // Verify Debug is implemented (would fail to compile otherwise)
        let _debug = format!("{:?}", cloned);
    }
}
