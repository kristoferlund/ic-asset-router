//! Proc-macro crate for `ic-asset-router`.
//!
//! Provides the `#[route]` attribute macro for per-route certification
//! configuration. This crate is re-exported by the main `ic-asset-router`
//! crate — users should not depend on it directly.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Ident, ItemFn, LitInt, LitStr, Token,
};

/// Attribute macro for per-route certification configuration.
///
/// Attach to a handler function to specify how its responses are certified.
/// The macro preserves the original function unchanged and generates a
/// sibling `__route_config()` function that returns the route configuration.
///
/// # Preset syntax
///
/// ```rust,ignore
/// #[route(certification = "skip")]
/// #[route(certification = "response_only")]
/// #[route(certification = "authenticated")]
/// ```
///
/// # Custom syntax
///
/// ```rust,ignore
/// #[route(certification = custom(
///     request_headers = ["authorization", "accept"],
///     query_params = ["page", "limit"],
///     response_headers = ["content-type"],
///     ttl = 300
/// ))]
/// ```
///
/// # Path override
///
/// ```rust,ignore
/// #[route(path = "custom-path")]
/// ```
///
/// The `path` argument is consumed by the build script's text scanner, not
/// by this macro. This macro ignores `path` if present alongside
/// `certification`.
#[proc_macro_attribute]
pub fn route(args: TokenStream, input: TokenStream) -> TokenStream {
    let route_args = parse_macro_input!(args as RouteArgs);
    let func = parse_macro_input!(input as ItemFn);

    let config_tokens = generate_route_config(&route_args);

    let expanded = quote! {
        #func

        #[doc(hidden)]
        pub fn __route_config() -> ic_asset_router::RouteConfig {
            #config_tokens
        }
    };

    expanded.into()
}

/// Parsed arguments from `#[route(...)]`.
struct RouteArgs {
    certification: Option<CertificationArg>,
    /// The `path` override is consumed by the build script's text scanner,
    /// not by the macro itself. We parse it to avoid a syn error but do not
    /// use it at expansion time.
    #[allow(dead_code)]
    path: Option<String>,
}

/// The certification argument value.
enum CertificationArg {
    /// A preset string: "skip", "response_only", "authenticated".
    Preset(String),
    /// Custom configuration with explicit fields.
    Custom(CustomCertConfig),
}

/// Custom certification configuration fields.
struct CustomCertConfig {
    request_headers: Vec<String>,
    query_params: Vec<String>,
    response_headers: Vec<String>,
    ttl: Option<u64>,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut certification = None;
        let mut path = None;

        let args = Punctuated::<RouteArg, Token![,]>::parse_terminated(input)?;
        for arg in args {
            match arg {
                RouteArg::Certification(c) => certification = Some(c),
                RouteArg::Path(p) => path = Some(p),
            }
        }

        Ok(RouteArgs {
            certification,
            path,
        })
    }
}

/// A single key-value argument in the `#[route(...)]` attribute.
enum RouteArg {
    Certification(CertificationArg),
    Path(String),
}

impl Parse for RouteArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        let _eq: Token![=] = input.parse()?;

        match key.to_string().as_str() {
            "certification" => {
                // Either a string literal preset or `custom(...)`.
                if input.peek(LitStr) {
                    let lit: LitStr = input.parse()?;
                    let value = lit.value();
                    match value.as_str() {
                        "skip" | "response_only" | "authenticated" => {
                            Ok(RouteArg::Certification(CertificationArg::Preset(value)))
                        }
                        other => Err(syn::Error::new(
                            lit.span(),
                            format!(
                                "unknown certification preset \"{other}\". \
                                 Expected \"skip\", \"response_only\", \"authenticated\", \
                                 or custom(...)"
                            ),
                        )),
                    }
                } else {
                    // Expect `custom(...)`.
                    let ident: Ident = input.parse()?;
                    if ident != "custom" {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!(
                                "expected \"skip\", \"response_only\", \"authenticated\", \
                                 or custom(...), found `{ident}`"
                            ),
                        ));
                    }
                    let content;
                    syn::parenthesized!(content in input);
                    let custom = parse_custom_config(&content)?;
                    Ok(RouteArg::Certification(CertificationArg::Custom(custom)))
                }
            }
            "path" => {
                let lit: LitStr = input.parse()?;
                Ok(RouteArg::Path(lit.value()))
            }
            other => Err(syn::Error::new(
                key.span(),
                format!(
                    "unknown route attribute key `{other}`. Expected `certification` or `path`"
                ),
            )),
        }
    }
}

/// Parse the interior of `custom(...)`.
fn parse_custom_config(input: ParseStream) -> syn::Result<CustomCertConfig> {
    let mut request_headers = Vec::new();
    let mut query_params = Vec::new();
    let mut response_headers = Vec::new();
    let mut ttl = None;

    let args = Punctuated::<CustomField, Token![,]>::parse_terminated(input)?;
    for field in args {
        match field {
            CustomField::RequestHeaders(v) => request_headers = v,
            CustomField::QueryParams(v) => query_params = v,
            CustomField::ResponseHeaders(v) => response_headers = v,
            CustomField::Ttl(v) => ttl = Some(v),
        }
    }

    Ok(CustomCertConfig {
        request_headers,
        query_params,
        response_headers,
        ttl,
    })
}

/// A single field inside `custom(...)`.
enum CustomField {
    RequestHeaders(Vec<String>),
    QueryParams(Vec<String>),
    ResponseHeaders(Vec<String>),
    Ttl(u64),
}

impl Parse for CustomField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        let _eq: Token![=] = input.parse()?;

        match key.to_string().as_str() {
            "request_headers" => {
                let values = parse_string_array(input)?;
                Ok(CustomField::RequestHeaders(values))
            }
            "query_params" => {
                let values = parse_string_array(input)?;
                Ok(CustomField::QueryParams(values))
            }
            "response_headers" => {
                let values = parse_string_array(input)?;
                Ok(CustomField::ResponseHeaders(values))
            }
            "ttl" => {
                let lit: LitInt = input.parse()?;
                let value: u64 = lit.base10_parse()?;
                Ok(CustomField::Ttl(value))
            }
            other => Err(syn::Error::new(
                key.span(),
                format!(
                    "unknown custom certification field `{other}`. Expected \
                     `request_headers`, `query_params`, `response_headers`, or `ttl`"
                ),
            )),
        }
    }
}

/// Parse a `["foo", "bar"]` array of string literals.
fn parse_string_array(input: ParseStream) -> syn::Result<Vec<String>> {
    let content;
    syn::bracketed!(content in input);
    let items = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
    Ok(items.iter().map(|lit| lit.value()).collect())
}

/// Generate the `RouteConfig { ... }` token stream from parsed args.
fn generate_route_config(args: &RouteArgs) -> proc_macro2::TokenStream {
    let cert_tokens = match &args.certification {
        None => {
            // No certification attribute — default to response_only.
            quote! { ic_asset_router::CertificationMode::response_only() }
        }
        Some(CertificationArg::Preset(preset)) => match preset.as_str() {
            "skip" => quote! { ic_asset_router::CertificationMode::skip() },
            "response_only" => quote! { ic_asset_router::CertificationMode::response_only() },
            "authenticated" => quote! { ic_asset_router::CertificationMode::authenticated() },
            _ => unreachable!("validated during parsing"),
        },
        Some(CertificationArg::Custom(config)) => {
            let mut builder_chain = quote! { ic_asset_router::FullConfig::builder() };

            if !config.request_headers.is_empty() {
                let headers: Vec<&str> =
                    config.request_headers.iter().map(|s| s.as_str()).collect();
                builder_chain = quote! {
                    #builder_chain.with_request_headers(&[#(#headers),*])
                };
            }

            if !config.query_params.is_empty() {
                let params: Vec<&str> = config.query_params.iter().map(|s| s.as_str()).collect();
                builder_chain = quote! {
                    #builder_chain.with_query_params(&[#(#params),*])
                };
            }

            if !config.response_headers.is_empty() {
                let headers: Vec<&str> =
                    config.response_headers.iter().map(|s| s.as_str()).collect();
                builder_chain = quote! {
                    #builder_chain.with_response_headers(&[#(#headers),*])
                };
            }

            quote! {
                ic_asset_router::CertificationMode::Full(#builder_chain.build())
            }
        }
    };

    let ttl_tokens = match &args.certification {
        Some(CertificationArg::Custom(config)) if config.ttl.is_some() => {
            let secs = config.ttl.unwrap();
            quote! { Some(std::time::Duration::from_secs(#secs)) }
        }
        _ => quote! { None },
    };

    quote! {
        ic_asset_router::RouteConfig {
            certification: #cert_tokens,
            ttl: #ttl_tokens,
            headers: vec![],
        }
    }
}
