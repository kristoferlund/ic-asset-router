/// Certification mode configuration for HTTP responses.
///
/// IC HTTP certification has three fundamental modes that determine which
/// parts of the HTTP request/response are hashed and cryptographically
/// certified. These types let you configure certification granularity
/// per-asset or per-route.
///
/// # Modes
///
/// - [`CertificationMode::Skip`] — No certification. Fastest, use for public
///   endpoints where tampering risk is acceptable.
/// - [`CertificationMode::ResponseOnly`] — Only the response is certified.
///   Good for static assets where the response depends only on the URL path.
/// - [`CertificationMode::Full`] — Both request and response are certified.
///   Required when the response depends on request headers (e.g.,
///   `Authorization`, `Accept`).

/// Certification mode for HTTP responses.
///
/// Determines which parts of the HTTP request/response are hashed and
/// certified. The default mode is [`CertificationMode::ResponseOnly`] with
/// a wildcard header inclusion and standard exclusions.
#[derive(Clone, Debug)]
pub enum CertificationMode {
    /// No certification. Response is served without cryptographic verification.
    /// Fastest option. Use for public endpoints where tampering risk is
    /// acceptable.
    Skip,

    /// Only the response is certified. Request details are not verified.
    /// Good for static assets where the response depends only on the URL path.
    ResponseOnly(ResponseOnlyConfig),

    /// Both request and response are certified.
    /// Required when the response depends on request headers (e.g.,
    /// `Authorization`, `Accept`).
    Full(FullConfig),
}

impl CertificationMode {
    /// Convenience constructor for skip mode.
    pub fn skip() -> Self {
        Self::Skip
    }

    /// Convenience constructor for response-only with default config.
    pub fn response_only() -> Self {
        Self::ResponseOnly(ResponseOnlyConfig::default())
    }

    /// Authenticated API: full certification with `Authorization` request
    /// header and `content-type` response header.
    ///
    /// Use this when the response depends on who is making the request.
    /// Different `Authorization` headers will produce different certified
    /// responses.
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
/// The response body and status code are always certified regardless of
/// header configuration.
#[derive(Clone, Debug)]
pub struct ResponseOnlyConfig {
    /// Response headers to include in certification hash.
    ///
    /// Use `vec!["*".to_string()]` to include all headers (with exclusions
    /// applied via [`exclude_headers`](Self::exclude_headers)).
    pub include_headers: Vec<String>,

    /// Response headers to explicitly exclude from certification.
    ///
    /// Applied after `include_headers`. Useful when `include_headers`
    /// contains `"*"`.
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
/// In full mode, the request method and body are **always** certified by
/// `ic-http-certification` — there is no opt-out. The configurable parts
/// are which request headers and query parameters participate in the
/// certification hash.
#[derive(Clone, Debug)]
pub struct FullConfig {
    /// Request headers to include in certification.
    /// Only these headers are hashed; others are ignored.
    pub request_headers: Vec<String>,

    /// Query parameters to include in certification.
    /// Only these params affect the certified response.
    pub query_params: Vec<String>,

    /// Response certification configuration.
    pub response: ResponseOnlyConfig,
}

impl Default for FullConfig {
    fn default() -> Self {
        Self {
            request_headers: vec![],
            query_params: vec![],
            response: ResponseOnlyConfig::default(),
        }
    }
}

impl FullConfig {
    /// Create a builder for ergonomic construction of [`FullConfig`].
    pub fn builder() -> FullConfigBuilder {
        FullConfigBuilder::default()
    }
}

/// Builder for [`FullConfig`] to enable ergonomic construction.
///
/// All `with_*_headers` methods normalize header names to lowercase.
///
/// # Example
///
/// ```
/// use ic_asset_router::certification::FullConfig;
///
/// let config = FullConfig::builder()
///     .with_request_headers(&["Authorization", "Accept"])
///     .with_query_params(&["page", "limit"])
///     .with_response_headers(&["Content-Type"])
///     .build();
///
/// assert_eq!(config.request_headers, vec!["authorization", "accept"]);
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
