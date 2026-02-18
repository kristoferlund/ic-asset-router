use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Reserved filenames that are never registered as route handlers.
///
/// These files have special semantics in the file-based routing convention:
/// - `middleware` — middleware function for the directory and its children
/// - `not_found` — custom 404 handler for unmatched routes
///
/// Files not in this list (including `all`, `index`) are treated as regular
/// route handlers.
///
/// # Escape hatch for reserved name collisions
///
/// If you need a route at a path that collides with a reserved filename
/// (e.g. `/middleware` or `/not_found`), use a differently-named file with a
/// `#[route(path = "...")]` attribute override:
///
/// ```text
/// // File: src/routes/mw_page.rs
/// #[route(path = "middleware")]
/// pub fn get(...) -> HttpResponse { ... }
/// ```
///
/// This registers a route at `/middleware` without conflicting with the
/// reserved `middleware.rs` convention.
const RESERVED_FILES: &[&str] = &["middleware", "not_found"];

/// Recognized HTTP method function names and their corresponding `Method` enum variants.
const METHOD_NAMES: &[(&str, &str)] = &[
    ("get", "Method::GET"),
    ("post", "Method::POST"),
    ("put", "Method::PUT"),
    ("patch", "Method::PATCH"),
    ("delete", "Method::DELETE"),
    ("head", "Method::HEAD"),
    ("options", "Method::OPTIONS"),
];

/// A detected method export from a route file.
struct MethodExport {
    /// The route path (e.g. "/api/users")
    route_path: String,
    /// The Rust module path to the handler function (e.g. "routes::api::users::get")
    handler_path: String,
    /// The `Method` variant string (e.g. "Method::GET")
    method_variant: String,
    /// Accumulated dynamic parameters for this route, in order from outermost
    /// to innermost. Empty for static routes.
    params: Vec<ParamMapping>,
    /// The Rust module path to the `Params` struct for this route (e.g.
    /// "routes::posts::_postId::Params"). `None` for routes without dynamic segments.
    params_type_path: Option<String>,
    /// The Rust module path to the `SearchParams` struct for this route (e.g.
    /// "routes::posts::index::SearchParams"). `None` for routes without typed
    /// search params.
    search_params_type_path: Option<String>,
    /// The Rust module path to the route file (e.g. "routes::api::users").
    /// Used to reference the generated `__route_config()` function.
    module_path: String,
    /// Whether the route file has a `#[route(certification = ...)]` attribute.
    /// When true, the generated route tree calls `module_path::__route_config()`.
    /// When false, `RouteConfig::default()` is used.
    has_certification_attribute: bool,
}

/// Mapping from a route param name to its struct field name.
#[derive(Clone)]
struct ParamMapping {
    /// The route-level parameter name (e.g. "postId" from `:postId`).
    route_name: String,
    /// The snake_case field name on the Params struct (e.g. "post_id").
    field_name: String,
}

/// A detected middleware file in a route directory.
struct MiddlewareExport {
    /// The middleware prefix (e.g. "/" for root, "/api" for api directory)
    prefix: String,
    /// The Rust module path to the middleware function (e.g. "routes::middleware::middleware")
    handler_path: String,
}

/// A detected `not_found.rs` file in the routes root directory.
struct NotFoundExport {
    /// The Rust module path to the handler function (e.g. "routes::not_found::get")
    handler_path: String,
}

/// Generate the route tree from the default `src/routes` directory.
///
/// This is a convenience wrapper around [`generate_routes_from`] that uses
/// `"src/routes"` as the routes directory. Call this from your crate's
/// `build.rs`:
///
/// ```rust,ignore
/// // build.rs
/// fn main() {
///     ic_asset_router::build::generate_routes();
/// }
/// ```
pub fn generate_routes() {
    generate_routes_from("src/routes");
}

/// Generate the route tree from a custom routes directory.
///
/// Scans `dir` recursively for `.rs` route files, generates handler wiring
/// code into `OUT_DIR/__route_tree.rs`, creates `mod.rs` files for IDE
/// visibility, and emits a `route_manifest.json` for debugging.
///
/// The `dir` parameter is the path to the routes directory relative to the
/// crate root (e.g. `"src/routes"` or `"src/api/routes"`).
///
/// # Routing conventions
///
/// | Filesystem pattern | URL route | Notes |
/// |--------------------|-----------|-------|
/// | `index.rs` | `/` (parent dir) | Index handler |
/// | `about.rs` | `/about` | Named route |
/// | `og.png.rs` | `/og.png` | Dotted filename — dots are preserved in the URL |
/// | `_postId/index.rs` | `/:postId` | Dynamic segment with typed `Params` struct |
/// | `all.rs` | `/*` | Catch-all wildcard |
/// | `middleware.rs` | — | Scoped middleware |
/// | `not_found.rs` | — | Custom 404 handler |
///
/// ## Dotted filenames
///
/// Files with dots before the `.rs` extension (e.g. `og.png.rs`, `feed.xml.rs`)
/// are served at the literal path including the dot (`/og.png`, `/feed.xml`).
/// The Rust module name replaces dots with underscores (`og_png`), and a
/// `#[path = "og.png.rs"]` attribute is emitted in the generated `mod.rs` so
/// the compiler can locate the source file.
pub fn generate_routes_from(dir: &str) {
    let routes_dir = Path::new(dir);
    let out_dir = std::env::var("OUT_DIR")
        .expect("OUT_DIR not set — this function must be called from a build script");
    let generated_file = Path::new(&out_dir).join("__route_tree.rs");

    // Tell Cargo to re-run the build script when any file in the routes
    // directory changes. We emit rerun-if-changed for the root and every
    // subdirectory so that adding/removing files anywhere in the tree
    // triggers a rebuild.
    println!("cargo:rerun-if-changed={dir}");
    fn emit_rerun_if_changed(dir: &Path) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());
            if path.is_dir() {
                emit_rerun_if_changed(&path);
            }
        }
    }
    emit_rerun_if_changed(routes_dir);

    let mut exports: Vec<MethodExport> = Vec::new();
    let mut middleware_exports: Vec<MiddlewareExport> = Vec::new();
    let mut not_found_exports: Vec<NotFoundExport> = Vec::new();
    process_directory(
        routes_dir,
        String::new(),
        &mut exports,
        &mut middleware_exports,
        &mut not_found_exports,
        &[],
    );

    // Sort by route path for deterministic output
    exports.sort_by(|a, b| {
        a.route_path
            .cmp(&b.route_path)
            .then(a.method_variant.cmp(&b.method_variant))
    });

    // Sort middleware by prefix for deterministic output
    middleware_exports.sort_by(|a, b| a.prefix.cmp(&b.prefix));

    let mut output = String::new();
    output.push_str("#[allow(unused_imports)]\n");
    output.push_str("use crate::routes;\n");
    output.push_str("#[allow(unused_imports)]\n");
    output.push_str("use ic_asset_router::Method;\n");
    output.push_str("#[allow(unused_imports)]\n");
    output.push_str("use ic_asset_router::router::{NodeType, RouteNode, RouteParams};\n");
    output.push_str("#[allow(unused_imports)]\n");
    output
        .push_str("use ic_asset_router::{HttpRequest, HttpResponse, RouteConfig, RouteContext, parse_query, deserialize_search_params};\n");
    output.push('\n');

    // Generate wrapper functions for each route handler.
    // Each wrapper bridges the router's internal (HttpRequest, RouteParams) signature
    // to the user-facing RouteContext<Params, SearchParams> signature.
    for (i, export) in exports.iter().enumerate() {
        let wrapper_name = format!("__route_handler_{i}");
        output.push_str("#[allow(unused_variables)]\n");
        output.push_str(&format!(
            "fn {wrapper_name}(req: HttpRequest, raw_params: RouteParams) -> HttpResponse<'static> {{\n"
        ));

        // Extract query string for both untyped (query) and typed (search) access.
        // Strips the fragment (#...) if present so serde_urlencoded sees clean input.
        output.push_str(
            "    let __query_str = req.url().split_once('?').map(|(_, q)| q.split_once('#').map_or(q, |(qs, _)| qs)).unwrap_or(\"\");\n",
        );

        // Deserialize typed search params if the route defines SearchParams.
        if let Some(ref search_path) = export.search_params_type_path {
            output.push_str(&format!(
                "    let __search: {search_path} = deserialize_search_params(__query_str);\n"
            ));
        }

        if let Some(ref params_path) = export.params_type_path {
            // Route has dynamic params — construct the typed Params struct.
            output.push_str("    let ctx = RouteContext {\n");
            output.push_str(&format!("        params: {params_path} {{\n"));
            for pm in &export.params {
                output.push_str(&format!(
                    "            {}: raw_params.get(\"{}\").cloned().unwrap_or_default(),\n",
                    pm.field_name, pm.route_name,
                ));
            }
            output.push_str("        },\n");
        } else {
            // Static route — use () as the params type.
            output.push_str("    let ctx = RouteContext {\n");
            output.push_str("        params: (),\n");
        }

        // Set the search field: typed SearchParams or () default.
        if export.search_params_type_path.is_some() {
            output.push_str("        search: __search,\n");
        } else {
            output.push_str("        search: (),\n");
        }

        output.push_str("        query: parse_query(req.url()),\n");
        output.push_str("        method: req.method().clone(),\n");
        output.push_str("        headers: req.headers().to_vec(),\n");
        output.push_str("        body: req.body().to_vec(),\n");
        output.push_str("        url: req.url().to_string(),\n");
        output.push_str("        wildcard: raw_params.get(\"*\").cloned(),\n");
        output.push_str("    };\n");
        output.push_str(&format!("    {}(ctx)\n", export.handler_path));
        output.push_str("}\n\n");
    }

    // Generate wrapper function for the not_found handler if present.
    if let Some(nf) = not_found_exports.first() {
        output.push_str("#[allow(unused_variables)]\n");
        output.push_str(
            "fn __not_found_handler(req: HttpRequest, raw_params: RouteParams) -> HttpResponse<'static> {\n",
        );
        output.push_str("    let ctx = RouteContext {\n");
        output.push_str("        params: (),\n");
        output.push_str("        search: (),\n");
        output.push_str("        query: parse_query(req.url()),\n");
        output.push_str("        method: req.method().clone(),\n");
        output.push_str("        headers: req.headers().to_vec(),\n");
        output.push_str("        body: req.body().to_vec(),\n");
        output.push_str("        url: req.url().to_string(),\n");
        output.push_str("        wildcard: None,\n");
        output.push_str("    };\n");
        output.push_str(&format!("    {}(ctx)\n", nf.handler_path));
        output.push_str("}\n\n");
    }

    output.push_str("thread_local! {\n");
    output.push_str("    pub static ROUTES: RouteNode = {\n");
    output.push_str("        let mut root = RouteNode::new(NodeType::Static(\"\".into()));\n");

    for (i, export) in exports.iter().enumerate() {
        output.push_str(&format!(
            "        root.insert(\"{}\", {}, __route_handler_{i});\n",
            export.route_path, export.method_variant,
        ));
    }

    // Generate set_route_config calls. Multiple methods on the same path share
    // the same config, so we deduplicate by route_path.
    {
        let mut seen_paths = std::collections::HashSet::new();
        for export in exports.iter() {
            if seen_paths.insert(export.route_path.clone()) {
                if export.has_certification_attribute {
                    output.push_str(&format!(
                        "        root.set_route_config(\"{}\", {}::__route_config());\n",
                        export.route_path, export.module_path,
                    ));
                } else {
                    output.push_str(&format!(
                        "        root.set_route_config(\"{}\", RouteConfig::default());\n",
                        export.route_path,
                    ));
                }
            }
        }
    }

    for mw in &middleware_exports {
        output.push_str(&format!(
            "        root.set_middleware(\"{}\", {});\n",
            mw.prefix, mw.handler_path,
        ));
    }

    // At most one not_found handler should be registered (from the root not_found.rs).
    if !not_found_exports.is_empty() {
        output.push_str("        root.set_not_found(__not_found_handler);\n");
    }

    output.push_str("        root\n    };\n}\n");

    let mut file = File::create(&generated_file).unwrap();
    file.write_all(output.as_bytes()).unwrap();

    // Generate route_manifest.json into OUT_DIR for debugging and inspection.
    let manifest_file = Path::new(&out_dir).join("route_manifest.json");
    let manifest = generate_manifest(&exports, &middleware_exports, &not_found_exports);
    fs::write(manifest_file, manifest).unwrap();
}

/// Generate a JSON route manifest listing all registered routes, middleware,
/// and the not-found handler. The manifest is intended for debugging and
/// tooling — it is not consumed by the Rust build.
fn generate_manifest(
    exports: &[MethodExport],
    middleware_exports: &[MiddlewareExport],
    not_found_exports: &[NotFoundExport],
) -> String {
    let mut json = String::from("{\n  \"routes\": [\n");

    for (i, export) in exports.iter().enumerate() {
        // Extract parameter names from the route path (segments starting with ':')
        let params: Vec<&str> = export
            .route_path
            .split('/')
            .filter(|s| s.starts_with(':'))
            .map(|s| &s[1..])
            .collect();

        // Extract the method name from the variant string (e.g. "Method::GET" → "GET")
        let method = export
            .method_variant
            .strip_prefix("Method::")
            .unwrap_or(&export.method_variant);

        json.push_str("    {\n");
        json.push_str(&format!(
            "      \"path\": \"{}\",\n",
            escape_json(&export.route_path)
        ));
        json.push_str(&format!(
            "      \"handler\": \"{}\",\n",
            escape_json(&export.handler_path)
        ));
        json.push_str(&format!("      \"method\": \"{method}\",\n"));

        json.push_str("      \"params\": [");
        for (j, param) in params.iter().enumerate() {
            json.push_str(&format!("\"{}\"", escape_json(param)));
            if j + 1 < params.len() {
                json.push_str(", ");
            }
        }
        json.push(']');

        json.push_str("\n    }");
        if i + 1 < exports.len() {
            json.push(',');
        }
        json.push('\n');
    }

    json.push_str("  ],\n  \"middleware\": [\n");

    for (i, mw) in middleware_exports.iter().enumerate() {
        json.push_str("    {\n");
        json.push_str(&format!(
            "      \"prefix\": \"{}\",\n",
            escape_json(&mw.prefix)
        ));
        json.push_str(&format!(
            "      \"handler\": \"{}\"\n",
            escape_json(&mw.handler_path)
        ));
        json.push_str("    }");
        if i + 1 < middleware_exports.len() {
            json.push(',');
        }
        json.push('\n');
    }

    json.push_str("  ],\n  \"not_found\": ");
    if let Some(nf) = not_found_exports.first() {
        json.push_str(&format!("\"{}\"", escape_json(&nf.handler_path)));
    } else {
        json.push_str("null");
    }
    json.push_str("\n}\n");

    json
}

/// Escape a string for JSON output (handles backslash and double-quote).
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A dynamic parameter accumulated from parent directories.
///
/// Tracks both the original camelCase name (for route path matching) and the
/// snake_case field name (for the generated `Params` struct).
struct AccumulatedParam {
    /// The original parameter name as it appears in the route path (e.g. "postId").
    route_name: String,
    /// The snake_case field name for the Params struct (e.g. "post_id").
    field_name: String,
}

fn process_directory(
    dir: &Path,
    prefix: String,
    exports: &mut Vec<MethodExport>,
    middleware_exports: &mut Vec<MiddlewareExport>,
    not_found_exports: &mut Vec<NotFoundExport>,
    accumulated_params: &[AccumulatedParam],
) {
    // Detect ambiguous routes: a file `_param.rs` and a directory `_param/` in
    // the same directory both map to the same route segment. This is an error.
    {
        let mut file_stems: Vec<String> = Vec::new();
        let mut dir_names: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                if path.is_dir() {
                    dir_names.push(name);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem != "mod" && stem != "index" {
                            file_stems.push(stem.to_string());
                        }
                    }
                }
            }
        }
        for stem in &file_stems {
            if dir_names.contains(stem) {
                panic!(
                    "Ambiguous route: both '{stem}.rs' and '{stem}/index.rs' exist in '{}'. \
                     Use one form or the other, not both.",
                    dir.display()
                );
            }
        }
    }

    let mut children = vec![];

    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap().to_str().unwrap();
            let next_prefix = if prefix.is_empty() {
                format!("/{name}")
            } else {
                format!("{prefix}/{name}")
            };
            fs::create_dir_all(&path).unwrap();
            // If this directory is a dynamic param (starts with `_`), accumulate
            // it for Params struct generation in child directories.
            let mut child_params: Vec<AccumulatedParam> = accumulated_params
                .iter()
                .map(|p| AccumulatedParam {
                    route_name: p.route_name.clone(),
                    field_name: p.field_name.clone(),
                })
                .collect();
            if let Some(param_name) = name.strip_prefix('_') {
                child_params.push(AccumulatedParam {
                    route_name: param_name.to_string(),
                    field_name: camel_to_snake(param_name),
                });
            }
            process_directory(
                &path,
                next_prefix,
                exports,
                middleware_exports,
                not_found_exports,
                &child_params,
            );
            let mod_name = sanitize_mod(name);
            if mod_name.starts_with('_') {
                children.push(format!("#[allow(non_snake_case)]\npub mod {mod_name};\n"));
            } else {
                children.push(format!("pub mod {mod_name};\n"));
            }
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let stem = path.file_stem().unwrap().to_str().unwrap();
            if stem == "mod" {
                continue;
            }

            // Reserved files are never registered as route handlers.
            // The RESERVED_FILES constant is the single source of truth.
            if RESERVED_FILES.contains(&stem) {
                match stem {
                    "middleware" => {
                        children.push("pub mod middleware;\n".to_string());
                        // Best-effort signature validation: warn if middleware.rs
                        // doesn't appear to export `pub fn middleware`.
                        if !has_pub_fn(&path, "middleware") {
                            println!(
                                "cargo:warning=middleware.rs in '{}' should export \
                                 `pub fn middleware(...)`. The generated wiring expects \
                                 this export and will fail to compile without it.",
                                dir.display()
                            );
                        }
                        let mw_prefix = if prefix.is_empty() {
                            "/".to_string()
                        } else {
                            prefix_to_route_path(&prefix)
                        };
                        let mw_handler_path = if prefix.is_empty() {
                            "routes::middleware::middleware".to_string()
                        } else {
                            let parts: Vec<String> = prefix
                                .split('/')
                                .filter(|s| !s.is_empty())
                                .map(sanitize_mod)
                                .collect();
                            format!("routes::{}::middleware::middleware", parts.join("::"))
                        };
                        middleware_exports.push(MiddlewareExport {
                            prefix: mw_prefix,
                            handler_path: mw_handler_path,
                        });
                    }
                    "not_found" => {
                        children.push("pub mod not_found;\n".to_string());
                        let methods = detect_method_exports(&path);
                        if methods.is_empty() {
                            panic!(
                                "not_found.rs does not export any recognized HTTP method functions (get, post, put, etc.). \
                                 It must export at least one."
                            );
                        }
                        // Use the `get` export if available, otherwise the first detected method.
                        let (fn_name, _) = methods
                            .iter()
                            .find(|(name, _)| *name == "get")
                            .unwrap_or(&methods[0]);
                        let handler_path = if prefix.is_empty() {
                            format!("routes::not_found::{fn_name}")
                        } else {
                            let parts: Vec<String> = prefix
                                .split('/')
                                .filter(|s| !s.is_empty())
                                .map(sanitize_mod)
                                .collect();
                            format!("routes::{}::not_found::{fn_name}", parts.join("::"))
                        };
                        not_found_exports.push(NotFoundExport { handler_path });
                    }
                    _ => {
                        // Future reserved filenames: skip route registration,
                        // emit a module declaration, and warn.
                        children.push(format!("pub mod {stem};\n"));
                        println!(
                            "cargo:warning=Reserved file '{stem}.rs' in '{}' was skipped — \
                             no handler registered for it.",
                            dir.display()
                        );
                    }
                }
                continue;
            }

            let mod_name = sanitize_mod(stem);
            // Check for a #[route(path = "...")] attribute override.
            // If present, use the attribute value as the route segment instead
            // of the filename-derived segment.
            let route_path = match scan_route_attribute(&path) {
                Some(override_segment) => {
                    // Build route path using the prefix + the override segment
                    let mut parts: Vec<String> = prefix
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .map(name_to_route_segment)
                        .collect();
                    parts.push(override_segment);
                    format!("/{}", parts.join("/"))
                }
                None => file_to_route_path(&prefix, stem),
            };
            let module_path = file_to_handler_path(&prefix, stem);

            // Emit module declaration. When the sanitized module name differs from the
            // filename stem (e.g. `og.png.rs` → mod `og_png`), add a `#[path]` attribute
            // so the compiler can find the source file.
            let path_attr = if mod_name != stem {
                format!("#[path = \"{stem}.rs\"]\n")
            } else {
                String::new()
            };
            if mod_name.starts_with('_') {
                children.push(format!(
                    "{path_attr}#[allow(non_snake_case)]\npub mod {mod_name};\n"
                ));
            } else {
                children.push(format!("{path_attr}pub mod {mod_name};\n"));
            }

            // Scan the file for recognized method exports
            let methods = detect_method_exports(&path);
            if methods.is_empty() {
                panic!(
                    "Route file '{}' does not export any recognized HTTP method functions (get, post, put, patch, delete, head, options). \
                     Each route file must export at least one.",
                    path.display()
                );
            }

            // Build params info for RouteContext wiring.
            let param_mappings: Vec<ParamMapping> = accumulated_params
                .iter()
                .map(|p| ParamMapping {
                    route_name: p.route_name.clone(),
                    field_name: p.field_name.clone(),
                })
                .collect();
            let params_type_path = if param_mappings.is_empty() {
                None
            } else {
                // The Params struct lives in the mod.rs of the directory that
                // contains this file. Build the module path from the prefix.
                let parts: Vec<String> = prefix
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .map(sanitize_mod)
                    .collect();
                if parts.is_empty() {
                    Some("routes::Params".to_string())
                } else {
                    Some(format!("routes::{}::Params", parts.join("::")))
                }
            };

            // Detect typed search params: if the route file defines
            // `pub struct SearchParams`, the generated wiring will
            // deserialize the query string into it via serde_urlencoded.
            let search_params_type_path = if has_search_params(&path) {
                Some(format!("{}::SearchParams", module_path))
            } else {
                None
            };

            // Detect #[route(certification = ...)] attribute for per-route
            // certification configuration.
            let has_cert_attr = scan_certification_attribute(&path);

            for (fn_name, variant) in &methods {
                exports.push(MethodExport {
                    route_path: route_path.clone(),
                    handler_path: format!("{}::{}", module_path, fn_name),
                    method_variant: variant.to_string(),
                    params: param_mappings.clone(),
                    params_type_path: params_type_path.clone(),
                    search_params_type_path: search_params_type_path.clone(),
                    module_path: module_path.clone(),
                    has_certification_attribute: has_cert_attr,
                });
            }
        }
    }

    if !children.is_empty() || !accumulated_params.is_empty() {
        let mut contents = String::new();

        // Generate the Params struct for IDE visibility if this directory has
        // accumulated dynamic parameters. Routes without dynamic segments use `()`.
        if !accumulated_params.is_empty() {
            contents.push_str("/// Typed route parameters for this route segment.\n");
            contents.push_str("///\n");
            contents.push_str("/// Auto-generated by the build script. Do not edit.\n");
            contents.push_str("#[derive(Debug, Clone)]\n");
            contents.push_str("pub struct Params {\n");
            for param in accumulated_params {
                contents.push_str(&format!("    pub {}: String,\n", param.field_name));
            }
            contents.push_str("}\n\n");
        }

        contents.push_str(&children.concat());

        let mod_path = dir.join("mod.rs");
        fs::write(mod_path, contents).unwrap();
    }
}

/// Scan a Rust source file for all `pub fn` declarations and return their names.
///
/// This is a best-effort text scan — not a full parser. It looks for lines
/// matching `pub fn <name>(` and extracts `<name>`. Used as the shared
/// implementation for [`has_pub_fn`] and [`detect_method_exports`].
fn scan_pub_fns(path: &Path) -> Vec<String> {
    let source = fs::read_to_string(path).unwrap_or_default();
    let prefix = "pub fn ";
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            // Find the function name: the identifier before '(' or whitespace.
            if let Some(paren_pos) = rest.find('(') {
                let name = rest[..paren_pos].trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Best-effort check: does the file contain `pub fn <name>(`?
///
/// Used for signature validation of reserved files (e.g. checking that
/// `middleware.rs` exports `pub fn middleware`). Delegates to [`scan_pub_fns`]
/// and checks if `name` is in the result.
fn has_pub_fn(path: &Path, name: &str) -> bool {
    scan_pub_fns(path).iter().any(|n| n == name)
}

/// Scan a Rust source file for `pub fn <method_name>` declarations matching
/// recognized HTTP methods. Returns a list of `(fn_name, Method_variant)` pairs.
///
/// Delegates to [`scan_pub_fns`] and filters against [`METHOD_NAMES`].
fn detect_method_exports(path: &Path) -> Vec<(&'static str, &'static str)> {
    let pub_fns = scan_pub_fns(path);
    METHOD_NAMES
        .iter()
        .filter(|(fn_name, _)| pub_fns.iter().any(|n| n == fn_name))
        .copied()
        .collect()
}

/// Scan a Rust source file for a `#[route(path = "...")]` attribute and return
/// the override path segment if present.
///
/// Parses the file with `syn` and walks top-level function items looking
/// for `#[route(...)]` attributes containing `path = "..."`. Handles
/// multi-line attributes correctly.
///
/// Returns `Some("ogimage.png")` if found, `None` otherwise.
fn scan_route_attribute(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    let file = syn::parse_file(&source).ok()?;
    for item in &file.items {
        if let syn::Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("route") {
                    let tokens = attr
                        .meta
                        .require_list()
                        .map(|list| list.tokens.to_string())
                        .ok()?;
                    // Parse `path = "value"` from the stringified tokens.
                    // The tokenizer normalizes whitespace, so we get `path = "value"`.
                    if let Some(rest) = tokens.strip_prefix("path") {
                        let rest = rest.trim_start();
                        if let Some(rest) = rest.strip_prefix('=') {
                            let rest = rest.trim();
                            if rest.starts_with('"') && rest.contains('"') {
                                // Extract the string between the first pair of quotes
                                let inner = &rest[1..];
                                if let Some(end) = inner.find('"') {
                                    return Some(inner[..end].to_string());
                                }
                            }
                        }
                    }
                    // Also handle when `path` is not the first argument:
                    // e.g., `certification = "skip" , path = "ogimage.png"`
                    if let Some(idx) = tokens.find("path") {
                        let rest = &tokens[idx + 4..];
                        let rest = rest.trim_start();
                        if let Some(rest) = rest.strip_prefix('=') {
                            let rest = rest.trim();
                            if rest.starts_with('"') {
                                let inner = &rest[1..];
                                if let Some(end) = inner.find('"') {
                                    return Some(inner[..end].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Scan a Rust source file for a `#[route(...)]` attribute that contains a
/// `certification` key.
///
/// Parses the file with `syn` and walks top-level function items looking
/// for `#[route(...)]` attributes. Returns `true` if any such attribute
/// contains a `certification` key. Handles multi-line attributes correctly.
fn scan_certification_attribute(path: &Path) -> bool {
    let source = fs::read_to_string(path).unwrap_or_default();
    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => return false,
    };
    for item in &file.items {
        if let syn::Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("route") {
                    let tokens = attr
                        .meta
                        .require_list()
                        .map(|list| list.tokens.to_string())
                        .unwrap_or_default();
                    if tokens.contains("certification") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Scan a Rust source file for a `pub struct SearchParams` declaration.
///
/// Parses the file with `syn` and walks top-level items looking for a
/// `pub struct SearchParams` declaration. Handles any formatting and
/// ignores private structs or structs in comments/string literals.
fn has_search_params(path: &Path) -> bool {
    let source = fs::read_to_string(path).unwrap_or_default();
    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => return false,
    };
    for item in &file.items {
        if let syn::Item::Struct(s) = item {
            if s.ident == "SearchParams" && matches!(s.vis, syn::Visibility::Public(_)) {
                return true;
            }
        }
    }
    false
}

/// Convert a camelCase identifier to snake_case.
///
/// Examples:
/// - `"postId"` → `"post_id"`
/// - `"userId"` → `"user_id"`
/// - `"id"` → `"id"`
/// - `"HTMLParser"` → `"html_parser"`
fn camel_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev_upper = chars[i - 1].is_uppercase();
                let next_lower = chars.get(i + 1).is_some_and(|nc| nc.is_lowercase());
                // Insert underscore before this uppercase letter if:
                // - previous char was lowercase (camelCase boundary), OR
                // - previous char was uppercase AND next char is lowercase
                //   (end of acronym like "HTMLParser" → insert before 'P')
                if !prev_upper || next_lower {
                    result.push('_');
                }
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

/// Sanitize a filesystem name into a valid Rust module identifier.
///
/// Dots are replaced with underscores so that dotted filenames like `og.png.rs`
/// produce valid module names (`og_png`). When the sanitized name differs from
/// the original, the caller emits a `#[path = "..."]` attribute.
fn sanitize_mod(name: &str) -> String {
    name.replace('.', "_")
}

/// Convert a raw filesystem prefix (e.g. `/_postId/edit`) to a route prefix
/// (e.g. `/:postId/edit`). Each segment is mapped through `name_to_route_segment`.
fn prefix_to_route_path(prefix: &str) -> String {
    let parts: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(name_to_route_segment)
        .collect();
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// Convert a filesystem name (file stem or directory name) to its route segment.
///
/// - `index` → `""` (maps to the parent directory path)
/// - `all` → `*` (catch-all wildcard)
/// - `_param` → `:param` (dynamic segment)
/// - anything else → literal segment
fn name_to_route_segment(name: &str) -> String {
    if name == "index" {
        String::new()
    } else if name == "all" {
        "*".to_string()
    } else if let Some(param) = name.strip_prefix('_') {
        format!(":{param}")
    } else {
        name.to_string()
    }
}

fn file_to_route_path(prefix: &str, name: &str) -> String {
    let mut parts: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(name_to_route_segment)
        .collect();

    let segment = name_to_route_segment(name);
    if !segment.is_empty() {
        parts.push(segment);
    }

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn file_to_handler_path(prefix: &str, name: &str) -> String {
    let mut parts: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(sanitize_mod)
        .collect();
    parts.push(sanitize_mod(name));
    format!("routes::{}", parts.join("::"))
}

// Test coverage audit (Session 7, Spec 5.5):
//
// Previously covered:
//   - camel_to_snake: simple, single word, user_id, multi-word, already snake, acronym, leading upper
//   - name_to_route_segment: index→"", all→"*", _param→":param", static→literal
//   - file_to_route_path: index at root, param directory, nested param directory
//   - has_search_params: detects pub struct, absent, ignores private struct
//
// Gaps filled in this session:
//   - scan_route_attribute: valid attribute, missing attribute, whitespace variations
//   - detect_method_exports: single method, multiple methods, no methods, near-miss names
//   - has_pub_fn: present, absent, near-miss names
//   - sanitize_mod: plain name, dotted name, underscore-prefixed
//   - prefix_to_route_path: root, single segment, param segment, nested
//   - file_to_handler_path: root, nested, param directory
//   - escape_json: plain, backslash, double-quote
//   - file_to_route_path: all→wildcard, static name
//   - RESERVED_FILES recognition
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_to_snake_simple() {
        assert_eq!(camel_to_snake("postId"), "post_id");
    }

    #[test]
    fn camel_to_snake_single_word() {
        assert_eq!(camel_to_snake("id"), "id");
    }

    #[test]
    fn camel_to_snake_user_id() {
        assert_eq!(camel_to_snake("userId"), "user_id");
    }

    #[test]
    fn camel_to_snake_multi_word() {
        assert_eq!(camel_to_snake("myLongParamName"), "my_long_param_name");
    }

    #[test]
    fn camel_to_snake_already_snake() {
        assert_eq!(camel_to_snake("post_id"), "post_id");
    }

    #[test]
    fn camel_to_snake_acronym() {
        assert_eq!(camel_to_snake("HTMLParser"), "html_parser");
    }

    #[test]
    fn camel_to_snake_leading_upper() {
        assert_eq!(camel_to_snake("PostId"), "post_id");
    }

    #[test]
    fn name_to_route_segment_index() {
        assert_eq!(name_to_route_segment("index"), "");
    }

    #[test]
    fn name_to_route_segment_all() {
        assert_eq!(name_to_route_segment("all"), "*");
    }

    #[test]
    fn name_to_route_segment_param() {
        assert_eq!(name_to_route_segment("_postId"), ":postId");
    }

    #[test]
    fn name_to_route_segment_static() {
        assert_eq!(name_to_route_segment("about"), "about");
    }

    #[test]
    fn file_to_route_path_index() {
        assert_eq!(file_to_route_path("", "index"), "/");
    }

    #[test]
    fn file_to_route_path_param_dir() {
        assert_eq!(file_to_route_path("/_postId", "edit"), "/:postId/edit");
    }

    #[test]
    fn file_to_route_path_nested() {
        assert_eq!(
            file_to_route_path("/posts/_postId", "index"),
            "/posts/:postId"
        );
    }

    // --- has_search_params tests ---

    fn write_temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("ic_asset_router_test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn has_search_params_detects_struct() {
        let path = write_temp_file(
            "sp_detect.rs",
            r#"
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct SearchParams {
    pub page: Option<u32>,
}
"#,
        );
        assert!(has_search_params(&path));
    }

    #[test]
    fn has_search_params_returns_false_when_absent() {
        let path = write_temp_file(
            "sp_absent.rs",
            r#"
pub fn get() -> String {
    "hello".to_string()
}
"#,
        );
        assert!(!has_search_params(&path));
    }

    #[test]
    fn has_search_params_ignores_private_struct() {
        let path = write_temp_file(
            "sp_private.rs",
            r#"
struct SearchParams {
    page: Option<u32>,
}
"#,
        );
        assert!(!has_search_params(&path));
    }

    // --- scan_route_attribute tests ---

    #[test]
    fn scan_route_attribute_basic() {
        let path = write_temp_file(
            "scan_basic.rs",
            r#"
#[route(path = "ogimage.png")]
pub fn get() -> String { todo!() }
"#,
        );
        assert_eq!(scan_route_attribute(&path), Some("ogimage.png".to_string()));
    }

    #[test]
    fn scan_route_attribute_with_spaces() {
        let path = write_temp_file(
            "scan_spaces.rs",
            r#"
#[route( path = "custom-name" )]
pub fn get() -> String { todo!() }
"#,
        );
        assert_eq!(scan_route_attribute(&path), Some("custom-name".to_string()));
    }

    #[test]
    fn scan_route_attribute_missing() {
        let path = write_temp_file(
            "scan_missing.rs",
            r#"
pub fn get() -> String { todo!() }
"#,
        );
        assert_eq!(scan_route_attribute(&path), None);
    }

    #[test]
    fn scan_route_attribute_non_route_attribute() {
        let path = write_temp_file(
            "scan_non_route.rs",
            r#"
#[derive(Debug)]
pub fn get() -> String { todo!() }
"#,
        );
        assert_eq!(scan_route_attribute(&path), None);
    }

    // --- detect_method_exports tests ---

    #[test]
    fn detect_method_exports_single_get() {
        let path = write_temp_file(
            "detect_get.rs",
            r#"
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> { todo!() }
"#,
        );
        let methods = detect_method_exports(&path);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].0, "get");
        assert_eq!(methods[0].1, "Method::GET");
    }

    #[test]
    fn detect_method_exports_multiple() {
        let path = write_temp_file(
            "detect_multi.rs",
            r#"
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> { todo!() }
pub fn post(ctx: RouteContext<()>) -> HttpResponse<'static> { todo!() }
pub fn delete(ctx: RouteContext<()>) -> HttpResponse<'static> { todo!() }
"#,
        );
        let methods = detect_method_exports(&path);
        assert_eq!(methods.len(), 3);
        let names: Vec<&str> = methods.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"get"));
        assert!(names.contains(&"post"));
        assert!(names.contains(&"delete"));
    }

    #[test]
    fn detect_method_exports_none() {
        let path = write_temp_file(
            "detect_none.rs",
            r#"
pub fn helper() -> String { todo!() }
"#,
        );
        let methods = detect_method_exports(&path);
        assert!(methods.is_empty());
    }

    #[test]
    fn detect_method_exports_no_false_match() {
        // `get_user` should not match `get`
        let path = write_temp_file(
            "detect_near_miss.rs",
            r#"
pub fn get_user(id: u64) -> String { todo!() }
"#,
        );
        let methods = detect_method_exports(&path);
        assert!(methods.is_empty());
    }

    #[test]
    fn detect_method_exports_private_fn_ignored() {
        // `fn get` without `pub` should not match
        let path = write_temp_file(
            "detect_private.rs",
            r#"
fn get(ctx: RouteContext<()>) -> HttpResponse<'static> { todo!() }
"#,
        );
        let methods = detect_method_exports(&path);
        assert!(methods.is_empty());
    }

    // --- has_pub_fn tests ---

    #[test]
    fn has_pub_fn_present() {
        let path = write_temp_file(
            "hpf_present.rs",
            r#"
pub fn middleware(req: HttpRequest, params: &RouteParams, next: &dyn Fn()) -> HttpResponse<'static> {
    todo!()
}
"#,
        );
        assert!(has_pub_fn(&path, "middleware"));
    }

    #[test]
    fn has_pub_fn_absent() {
        let path = write_temp_file(
            "hpf_absent.rs",
            r#"
pub fn handler() -> String { todo!() }
"#,
        );
        assert!(!has_pub_fn(&path, "middleware"));
    }

    #[test]
    fn has_pub_fn_near_miss() {
        // `pub fn middleware_v2` should not match `middleware`
        let path = write_temp_file(
            "hpf_near_miss.rs",
            r#"
pub fn middleware_v2(req: HttpRequest) -> HttpResponse<'static> { todo!() }
"#,
        );
        assert!(!has_pub_fn(&path, "middleware"));
    }

    // --- sanitize_mod tests ---

    #[test]
    fn sanitize_mod_plain() {
        assert_eq!(sanitize_mod("about"), "about");
    }

    #[test]
    fn sanitize_mod_dot_replacement() {
        assert_eq!(sanitize_mod("file.name"), "file_name");
    }

    #[test]
    fn sanitize_mod_underscore_prefixed() {
        assert_eq!(sanitize_mod("_postId"), "_postId");
    }

    // --- prefix_to_route_path tests ---

    #[test]
    fn prefix_to_route_path_empty() {
        assert_eq!(prefix_to_route_path(""), "/");
    }

    #[test]
    fn prefix_to_route_path_single() {
        assert_eq!(prefix_to_route_path("/api"), "/api");
    }

    #[test]
    fn prefix_to_route_path_param() {
        assert_eq!(prefix_to_route_path("/_postId"), "/:postId");
    }

    #[test]
    fn prefix_to_route_path_nested() {
        assert_eq!(
            prefix_to_route_path("/api/_userId/posts"),
            "/api/:userId/posts"
        );
    }

    // --- file_to_handler_path tests ---

    #[test]
    fn file_to_handler_path_root() {
        assert_eq!(file_to_handler_path("", "index"), "routes::index");
    }

    #[test]
    fn file_to_handler_path_nested() {
        assert_eq!(
            file_to_handler_path("/api/users", "index"),
            "routes::api::users::index"
        );
    }

    #[test]
    fn file_to_handler_path_param_dir() {
        assert_eq!(
            file_to_handler_path("/_postId", "edit"),
            "routes::_postId::edit"
        );
    }

    // --- file_to_route_path additional tests ---

    #[test]
    fn file_to_route_path_all_wildcard() {
        assert_eq!(file_to_route_path("/files", "all"), "/files/*");
    }

    #[test]
    fn file_to_route_path_static_name() {
        assert_eq!(file_to_route_path("", "about"), "/about");
    }

    #[test]
    fn file_to_route_path_deeply_nested() {
        assert_eq!(
            file_to_route_path("/api/v2/_userId/posts", "index"),
            "/api/v2/:userId/posts"
        );
    }

    // --- escape_json tests ---

    #[test]
    fn escape_json_plain() {
        assert_eq!(escape_json("hello world"), "hello world");
    }

    #[test]
    fn escape_json_backslash() {
        assert_eq!(escape_json("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_json_quote() {
        assert_eq!(escape_json(r#"say "hi""#), r#"say \"hi\""#);
    }

    // --- RESERVED_FILES tests ---

    #[test]
    fn reserved_files_contains_middleware() {
        assert!(RESERVED_FILES.contains(&"middleware"));
    }

    #[test]
    fn reserved_files_contains_not_found() {
        assert!(RESERVED_FILES.contains(&"not_found"));
    }

    #[test]
    fn reserved_files_does_not_contain_index() {
        assert!(!RESERVED_FILES.contains(&"index"));
    }

    #[test]
    fn reserved_files_does_not_contain_all() {
        assert!(!RESERVED_FILES.contains(&"all"));
    }

    // --- process_directory integration tests (using temp dirs) ---

    /// RAII guard for a temporary route directory. Cleans up the directory
    /// tree on drop so tests do not leak temp files.
    struct TempRouteDir {
        path: std::path::PathBuf,
    }

    impl TempRouteDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRouteDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Helper: create a temp directory tree and return a guard that cleans up on drop.
    fn setup_temp_routes(structure: &[(&str, &str)]) -> TempRouteDir {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir()
            .join("ic_asset_router_test")
            .join(format!("routes_{id}"));
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        fs::create_dir_all(&base).unwrap();
        for (path, content) in structure {
            let full = base.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full, content).unwrap();
        }
        TempRouteDir { path: base }
    }

    #[test]
    fn process_directory_basic_index() {
        let dir = setup_temp_routes(&[("index.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].route_path, "/");
        assert_eq!(exports[0].method_variant, "Method::GET");
    }

    #[test]
    fn process_directory_static_route() {
        let dir = setup_temp_routes(&[("about.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].route_path, "/about");
    }

    #[test]
    fn process_directory_param_directory() {
        let dir = setup_temp_routes(&[("_postId/index.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].route_path, "/:postId");
        assert!(exports[0].params_type_path.is_some());
    }

    #[test]
    fn process_directory_wildcard_all() {
        let dir = setup_temp_routes(&[("all.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].route_path, "/*");
    }

    #[test]
    fn process_directory_middleware_detected() {
        let dir = setup_temp_routes(&[(
            "middleware.rs",
            "pub fn middleware(req: R, params: &P, next: &dyn Fn()) -> R { todo!() }",
        )]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        // middleware.rs should NOT be registered as a route
        assert!(exports.is_empty());
        // But it should be registered as middleware
        assert_eq!(mw.len(), 1);
        assert_eq!(mw[0].prefix, "/");
    }

    #[test]
    fn process_directory_not_found_detected() {
        let dir = setup_temp_routes(&[("not_found.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        // not_found.rs should NOT be registered as a route
        assert!(exports.is_empty());
        // But it should be registered as not-found handler
        assert_eq!(nf.len(), 1);
    }

    #[test]
    fn process_directory_nested_structure() {
        let dir = setup_temp_routes(&[
            ("index.rs", "pub fn get() -> () { todo!() }"),
            ("about.rs", "pub fn get() -> () { todo!() }"),
            ("posts/_postId/index.rs", "pub fn get() -> () { todo!() }"),
            (
                "posts/_postId/edit.rs",
                "pub fn get() -> () { todo!() }\npub fn post() -> () { todo!() }",
            ),
        ]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );

        let paths: Vec<&str> = exports.iter().map(|e| e.route_path.as_str()).collect();
        assert!(paths.contains(&"/"));
        assert!(paths.contains(&"/about"));
        assert!(paths.contains(&"/posts/:postId"));
        assert!(paths.contains(&"/posts/:postId/edit"));

        // edit.rs should produce both GET and POST
        let edit_methods: Vec<&str> = exports
            .iter()
            .filter(|e| e.route_path == "/posts/:postId/edit")
            .map(|e| e.method_variant.as_str())
            .collect();
        assert!(edit_methods.contains(&"Method::GET"));
        assert!(edit_methods.contains(&"Method::POST"));
    }

    #[test]
    fn process_directory_route_attribute_override() {
        let dir = setup_temp_routes(&[(
            "og_image.rs",
            "#[route(path = \"ogimage.png\")]\npub fn get() -> () { todo!() }",
        )]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].route_path, "/ogimage.png");
    }

    #[test]
    #[should_panic(expected = "Ambiguous route")]
    fn process_directory_ambiguous_route_panics() {
        let dir = setup_temp_routes(&[
            ("_param.rs", "pub fn get() -> () { todo!() }"),
            ("_param/index.rs", "pub fn get() -> () { todo!() }"),
        ]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
    }

    #[test]
    #[should_panic(expected = "does not export any recognized HTTP method")]
    fn process_directory_route_without_methods_panics() {
        let dir = setup_temp_routes(&[("broken.rs", "pub fn helper() -> String { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
    }

    #[test]
    fn process_directory_empty_dir() {
        let dir = setup_temp_routes(&[]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert!(exports.is_empty());
        assert!(mw.is_empty());
        assert!(nf.is_empty());
    }

    #[test]
    fn process_directory_search_params_detected() {
        let dir = setup_temp_routes(&[(
            "search.rs",
            r#"
use serde::Deserialize;
#[derive(Deserialize, Default)]
pub struct SearchParams {
    pub q: Option<String>,
}
pub fn get() -> () { todo!() }
"#,
        )]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert!(exports[0].search_params_type_path.is_some());
    }

    // --- scan_certification_attribute tests ---

    #[test]
    fn scan_certification_attribute_skip() {
        let path = write_temp_file(
            "cert_skip.rs",
            r#"
#[route(certification = "skip")]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_response_only() {
        let path = write_temp_file(
            "cert_ro.rs",
            r#"
#[route(certification = "response_only")]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_authenticated() {
        let path = write_temp_file(
            "cert_auth.rs",
            r#"
#[route(certification = "authenticated")]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_custom() {
        let path = write_temp_file(
            "cert_custom.rs",
            r#"
#[route(certification = custom(request_headers = ["authorization"]))]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_absent() {
        let path = write_temp_file(
            "cert_absent.rs",
            r#"
pub fn get() -> () { todo!() }
"#,
        );
        assert!(!scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_path_only() {
        let path = write_temp_file(
            "cert_path_only.rs",
            r#"
#[route(path = "ogimage.png")]
pub fn get() -> () { todo!() }
"#,
        );
        // Has #[route(...)] but no certification key
        assert!(!scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_attribute_multiline() {
        let path = write_temp_file(
            "cert_multiline.rs",
            r#"
#[route(
    certification = custom(
        request_headers = ["authorization"],
        query_params = ["page", "limit"]
    )
)]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(scan_certification_attribute(&path));
    }

    #[test]
    fn scan_certification_in_comment_ignored() {
        let path = write_temp_file(
            "cert_comment.rs",
            r#"
// #[route(certification = "skip")]
pub fn get() -> () { todo!() }
"#,
        );
        assert!(!scan_certification_attribute(&path));
    }

    #[test]
    fn has_search_params_multiline_struct() {
        let path = write_temp_file(
            "sp_multiline.rs",
            r#"
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct SearchParams
{
    pub page: Option<u32>,
    pub limit: Option<u32>,
}
"#,
        );
        assert!(has_search_params(&path));
    }

    #[test]
    fn scan_route_attribute_multiline() {
        let path = write_temp_file(
            "scan_multiline.rs",
            r#"
#[route(
    path = "ogimage.png",
    certification = "skip"
)]
pub fn get() -> () { todo!() }
"#,
        );
        assert_eq!(scan_route_attribute(&path), Some("ogimage.png".to_string()));
    }

    // --- process_directory certification detection tests ---

    #[test]
    fn process_directory_detects_certification_attribute() {
        let dir = setup_temp_routes(&[(
            "api.rs",
            "#[route(certification = \"skip\")]\npub fn get() -> () { todo!() }",
        )]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert!(exports[0].has_certification_attribute);
    }

    #[test]
    fn process_directory_no_certification_attribute() {
        let dir = setup_temp_routes(&[("about.rs", "pub fn get() -> () { todo!() }")]);
        let mut exports = Vec::new();
        let mut mw = Vec::new();
        let mut nf = Vec::new();
        process_directory(
            dir.path(),
            String::new(),
            &mut exports,
            &mut mw,
            &mut nf,
            &[],
        );
        assert_eq!(exports.len(), 1);
        assert!(!exports[0].has_certification_attribute);
    }
}
