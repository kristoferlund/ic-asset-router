#[cfg(test)]
mod tests {
    use candid::Principal;
    use pocket_ic::PocketIc;
    use reqwest::blocking::Client;
    use std::time::Duration;

    /// Path to the pre-built test canister WASM.
    /// The build_and_test.sh script compiles this before running the tests.
    const WASM_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_canister/target/wasm32-unknown-unknown/release/test_canister.wasm"
    );

    /// Deploy the test canister to a fresh PocketIC instance and start the HTTP gateway.
    /// Returns `(pic, client, gateway_url, canister_id)`.
    fn setup() -> (PocketIc, Client, String, Principal) {
        let mut pic = PocketIc::new();
        let canister_id = pic.create_canister();
        pic.add_cycles(canister_id, 2_000_000_000_000);

        let wasm = std::fs::read(WASM_PATH)
            .expect("test canister WASM not found — run build_and_test.sh to compile it first");
        pic.install_canister(canister_id, wasm, vec![], None);

        let gateway_url = pic.make_live(None);
        let base = format!("http://localhost:{}", gateway_url.port().unwrap());

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        // The HTTP gateway routes requests to the canister via the Host header
        // or the `canisterId` query parameter. We use the query parameter approach.
        let base_url = format!("{}/?canisterId={}", base, canister_id);

        (pic, client, base_url, canister_id)
    }

    /// Build a URL for a specific path on the test canister.
    /// The canisterId query param is appended for HTTP gateway routing.
    fn url_for(base: &str, path: &str) -> String {
        // base already has `/?canisterId=<id>`, so we need to construct
        // `http://host:port/<path>?canisterId=<id>`
        let parts: Vec<&str> = base.splitn(2, '?').collect();
        let origin = parts[0].trim_end_matches('/');
        let query = parts.get(1).unwrap_or(&"");
        let clean_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
        if query.is_empty() {
            format!("{}{}", origin, clean_path)
        } else {
            format!("{}{}?{}", origin, clean_path, query)
        }
    }

    // -----------------------------------------------------------------------
    // 5.7.9 — Static asset serving, dynamic route first request, cached response
    // -----------------------------------------------------------------------

    #[test]
    fn test_static_asset_serving() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client.get(url_for(&base_url, "/style.css")).send().unwrap();
        assert_eq!(
            resp.status().as_u16(),
            200,
            "static asset should return 200"
        );

        let ct = resp
            .headers()
            .get("content-type")
            .expect("content-type header missing")
            .to_str()
            .unwrap()
            .to_string();
        assert!(ct.contains("text/css"), "expected text/css, got: {ct}");

        // Certification header should be present
        assert!(
            resp.headers().get("ic-certificate").is_some(),
            "IC-Certificate header missing on static asset response"
        );
    }

    #[test]
    fn test_dynamic_route_first_request() {
        let (_pic, client, base_url, _cid) = setup();

        // First request to GET / — triggers query→update flow
        let resp = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp.status().as_u16(), 200, "GET / should return 200");

        let body = resp.text().unwrap();
        assert_eq!(body, "hello", "GET / should return 'hello'");
    }

    #[test]
    fn test_dynamic_route_cached_response() {
        let (_pic, client, base_url, _cid) = setup();

        // First request to populate the cache
        let resp1 = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp1.status().as_u16(), 200);
        let body1 = resp1.text().unwrap();

        // Second request should be served from cache
        let resp2 = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp2.status().as_u16(), 200);
        let body2 = resp2.text().unwrap();

        assert_eq!(body1, body2, "cached response should match first response");

        // Certification header should be present on cached response
        // (we check on a fresh request since we consumed resp2 for text)
        let resp3 = client.get(url_for(&base_url, "/")).send().unwrap();
        assert!(
            resp3.headers().get("ic-certificate").is_some(),
            "IC-Certificate header missing on cached response"
        );
    }

    // -----------------------------------------------------------------------
    // 5.7.10 — Parameter extraction, wildcard, JSON, method dispatch
    // -----------------------------------------------------------------------

    #[test]
    fn test_parameter_extraction_posts() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client.get(url_for(&base_url, "/posts/42")).send().unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let body = resp.text().unwrap();
        assert!(
            body.contains("42"),
            "GET /posts/42 should contain '42', got: {body}"
        );
    }

    #[test]
    fn test_parameter_extraction_echo() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client
            .get(url_for(&base_url, "/echo/some-value"))
            .send()
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let body = resp.text().unwrap();
        assert_eq!(
            body, "some-value",
            "GET /echo/some-value should return 'some-value'"
        );
    }

    #[test]
    fn test_wildcard_capture() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client
            .get(url_for(&base_url, "/files/docs/2024/report.pdf"))
            .send()
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let body = resp.text().unwrap();
        assert!(
            body.contains("docs/2024/report.pdf"),
            "GET /files/docs/2024/report.pdf should contain 'docs/2024/report.pdf', got: {body}"
        );
    }

    #[test]
    fn test_json_content_type() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client.get(url_for(&base_url, "/json")).send().unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let ct = resp
            .headers()
            .get("content-type")
            .expect("content-type header missing")
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            ct.contains("application/json"),
            "expected application/json, got: {ct}"
        );

        let body = resp.text().unwrap();
        assert_eq!(body, r#"{"ok":true}"#, "JSON body mismatch");
    }

    #[test]
    fn test_http_method_dispatch_get() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client
            .get(url_for(&base_url, "/method_test"))
            .send()
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp.text().unwrap(), "get");
    }

    #[test]
    fn test_http_method_dispatch_post() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client
            .post(url_for(&base_url, "/method_test"))
            .send()
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp.text().unwrap(), "post");
    }

    #[test]
    fn test_http_method_dispatch_405() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client
            .put(url_for(&base_url, "/method_test"))
            .send()
            .unwrap();
        assert_eq!(
            resp.status().as_u16(),
            405,
            "PUT /method_test should return 405"
        );
    }

    // -----------------------------------------------------------------------
    // 5.7.11 — Security headers, custom 404, middleware
    // -----------------------------------------------------------------------

    #[test]
    fn test_security_headers_present() {
        let (_pic, client, base_url, _cid) = setup();

        // First request triggers the query→update certification flow.
        // The update-path response is the raw handler output; security headers
        // are baked into the IC asset certification config for future queries.
        let resp1 = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp1.status().as_u16(), 200);

        // Second request is served from the certified cache via
        // asset_router.serve_asset(), which includes the merged security headers.
        let resp2 = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp2.status().as_u16(), 200);

        let headers = resp2.headers();
        assert!(
            headers.get("x-content-type-options").is_some(),
            "x-content-type-options header missing on cached response"
        );
        assert!(
            headers.get("x-frame-options").is_some(),
            "x-frame-options header missing on cached response"
        );
    }

    #[test]
    fn test_custom_404_handler() {
        let (_pic, client, base_url, _cid) = setup();

        // First request to an unknown path: triggers the query→update flow.
        // The update path runs the not-found handler and certifies the 404
        // response via certify_dynamic_response (spec 6.1).
        let resp1 = client
            .get(url_for(&base_url, "/nonexistent"))
            .send()
            .unwrap();
        assert_eq!(
            resp1.status().as_u16(),
            404,
            "GET /nonexistent should return 404"
        );
        let body1 = resp1.text().unwrap();
        assert!(
            body1.contains("custom 404"),
            "custom 404 handler should produce body containing 'custom 404', got: {body1}"
        );

        // Second request: served from the certified cache.
        let resp2 = client
            .get(url_for(&base_url, "/nonexistent"))
            .send()
            .unwrap();
        assert_eq!(
            resp2.status().as_u16(),
            404,
            "cached 404 should still return 404"
        );
        assert!(
            resp2.headers().get("ic-certificate").is_some(),
            "IC-Certificate header should be present on cached 404 response"
        );
        let body2 = resp2.text().unwrap();
        assert!(
            body2.contains("custom 404"),
            "cached 404 body should still contain 'custom 404', got: {body2}"
        );
    }

    #[test]
    fn test_middleware_header_injection() {
        let (_pic, client, base_url, _cid) = setup();

        let resp = client.get(url_for(&base_url, "/")).send().unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let mw_header = resp
            .headers()
            .get("x-test-middleware")
            .map(|v| v.to_str().unwrap().to_string());
        assert_eq!(
            mw_header.as_deref(),
            Some("applied"),
            "x-test-middleware header should be 'applied'"
        );
    }

    // -----------------------------------------------------------------------
    // 5.7.12 — Cache invalidation and TTL expiry (Phase 4 features)
    // -----------------------------------------------------------------------

    #[test]
    fn test_cache_invalidation_via_update_call() {
        let (pic, client, base_url, canister_id) = setup();

        // First request: triggers update, response is cached.
        let resp1 = client.get(url_for(&base_url, "/posts/42")).send().unwrap();
        assert_eq!(resp1.status().as_u16(), 200);
        let body1 = resp1.text().unwrap();
        assert!(body1.contains("42"), "first response should contain '42'");

        // Second request: served from cache (same body).
        let resp2 = client.get(url_for(&base_url, "/posts/42")).send().unwrap();
        assert_eq!(resp2.status().as_u16(), 200);
        let body2 = resp2.text().unwrap();
        assert_eq!(body1, body2, "cached response should match first response");

        // Invalidate the cached path via a Candid update call.
        let invalidate_arg = candid::encode_one("/posts/42".to_string()).unwrap();
        pic.update_call(
            canister_id,
            Principal::anonymous(),
            "invalidate",
            invalidate_arg,
        )
        .expect("invalidate call should succeed");

        // Third request: cache was invalidated, should trigger a new update call
        // and return a valid response.
        let resp3 = client.get(url_for(&base_url, "/posts/42")).send().unwrap();
        assert_eq!(resp3.status().as_u16(), 200);
        let body3 = resp3.text().unwrap();
        assert!(
            body3.contains("42"),
            "response after invalidation should still contain '42'"
        );
    }

    #[test]
    fn test_ttl_expiry() {
        let (pic, client, base_url, _cid) = setup();

        // First request to /ttl_test: triggers update, response is cached.
        // The handler returns the current IC time as a string.
        let resp1 = client.get(url_for(&base_url, "/ttl_test")).send().unwrap();
        assert_eq!(resp1.status().as_u16(), 200);
        let body1 = resp1.text().unwrap();
        assert!(!body1.is_empty(), "response body should not be empty");

        // Second request: should be served from cache (same timestamp).
        let resp2 = client.get(url_for(&base_url, "/ttl_test")).send().unwrap();
        assert_eq!(resp2.status().as_u16(), 200);
        let body2 = resp2.text().unwrap();
        assert_eq!(body1, body2, "cached response should match first response");

        // Advance time past the 5-second TTL configured for /ttl_test.
        pic.advance_time(Duration::from_secs(10));
        pic.tick();

        // Third request: TTL expired, should trigger a new update call with
        // a fresh IC time. The response should be a valid 200 with a (potentially
        // different) timestamp.
        let resp3 = client.get(url_for(&base_url, "/ttl_test")).send().unwrap();
        assert_eq!(resp3.status().as_u16(), 200);
        let body3 = resp3.text().unwrap();
        assert!(
            !body3.is_empty(),
            "regenerated response should not be empty"
        );

        // The new timestamp should differ from the cached one because IC time
        // has advanced.
        assert_ne!(
            body1, body3,
            "response after TTL expiry should have a different timestamp"
        );
    }
}
