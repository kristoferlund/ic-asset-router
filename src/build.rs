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

/// Generates a route tree from the default `src/routes` directory.
///
/// This is a convenience wrapper around [`generate_routes_from`] for backwards
/// compatibility.
pub fn generate_routes() {
    generate_routes_from("src/routes");
}

/// Generates a route tree from the specified routes directory and writes it to a
/// file. Also ensures that `mod.rs` files are created in each directory.
///
/// The `dir` parameter is the path to the routes directory relative to the
/// crate root (e.g. `"src/routes"` or `"src/api/routes"`).
pub fn generate_routes_from(dir: &str) {
    let routes_dir = Path::new(dir);
    let out_dir = std::env::var("OUT_DIR")
        .expect("OUT_DIR not set — this function must be called from a build script");
    let generated_file = Path::new(&out_dir).join("__route_tree.rs");

    // Tell Cargo to re-run the build script when any file in the routes
    // directory changes.
    println!("cargo:rerun-if-changed={dir}");

    let mut exports: Vec<MethodExport> = Vec::new();
    let mut middleware_exports: Vec<MiddlewareExport> = Vec::new();
    let mut not_found_exports: Vec<NotFoundExport> = Vec::new();
    process_directory(
        routes_dir,
        String::new(),
        &mut exports,
        &mut middleware_exports,
        &mut not_found_exports,
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
    output.push_str("use crate::routes;\n");
    output.push_str("use ic_http_certification::Method;\n");
    output.push_str("use router_library::router::{NodeType, RouteNode};\n\n");
    output.push_str("thread_local! {\n");
    output.push_str("    pub static ROUTES: RouteNode = {\n");
    output.push_str("        let mut root = RouteNode::new(NodeType::Static(\"\".into()));\n");

    for export in &exports {
        output.push_str(&format!(
            "        root.insert(\"{}\", {}, {});\n",
            export.route_path, export.method_variant, export.handler_path,
        ));
    }

    for mw in &middleware_exports {
        output.push_str(&format!(
            "        root.set_middleware(\"{}\", {});\n",
            mw.prefix, mw.handler_path,
        ));
    }

    // At most one not_found handler should be registered (from the root not_found.rs).
    if let Some(nf) = not_found_exports.first() {
        output.push_str(&format!(
            "        root.set_not_found({});\n",
            nf.handler_path,
        ));
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

fn process_directory(
    dir: &Path,
    prefix: String,
    exports: &mut Vec<MethodExport>,
    middleware_exports: &mut Vec<MiddlewareExport>,
    not_found_exports: &mut Vec<NotFoundExport>,
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
            process_directory(
                &path,
                next_prefix,
                exports,
                middleware_exports,
                not_found_exports,
            );
            children.push(format!("pub mod {};\n", sanitize_mod(name)));
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
                                .map(|s| sanitize_mod(s))
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
                                .map(|s| sanitize_mod(s))
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
                        .map(|s| name_to_route_segment(s))
                        .collect();
                    parts.push(override_segment);
                    format!("/{}", parts.join("/"))
                }
                None => file_to_route_path(&prefix, stem),
            };
            let module_path = file_to_handler_path(&prefix, stem);

            // All filenames are valid Rust identifiers with the new naming convention
            // (_param, all, etc.) — no #[path = "..."] attributes needed.
            children.push(format!("pub mod {mod_name};\n"));

            // Scan the file for recognized method exports
            let methods = detect_method_exports(&path);
            if methods.is_empty() {
                panic!(
                    "Route file '{}' does not export any recognized HTTP method functions (get, post, put, patch, delete, head, options). \
                     Each route file must export at least one.",
                    path.display()
                );
            }

            for (fn_name, variant) in &methods {
                exports.push(MethodExport {
                    route_path: route_path.clone(),
                    handler_path: format!("{}::{}", module_path, fn_name),
                    method_variant: variant.to_string(),
                });
            }
        }
    }

    if !children.is_empty() {
        let contents: String = children.concat();
        let mod_path = dir.join("mod.rs");
        fs::write(mod_path, contents).unwrap();
    }
}

/// Best-effort check: does the file contain `pub fn <name>(`?
///
/// Used for signature validation of reserved files (e.g. checking that
/// `middleware.rs` exports `pub fn middleware`). Not a full parser — just
/// scans lines for the expected pattern.
fn has_pub_fn(path: &Path, name: &str) -> bool {
    let source = fs::read_to_string(path).unwrap_or_default();
    let pattern = format!("pub fn {name}");
    source.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            let rest = &trimmed[pattern.len()..].trim_start();
            rest.starts_with('(')
        } else {
            false
        }
    })
}

/// Scan a Rust source file for `pub fn <method_name>` declarations matching
/// recognized HTTP methods. Returns a list of `(fn_name, Method_variant)` pairs.
fn detect_method_exports(path: &Path) -> Vec<(&'static str, &'static str)> {
    let source = fs::read_to_string(path).unwrap_or_default();
    let mut found = Vec::new();

    for &(fn_name, variant) in METHOD_NAMES {
        // Match `pub fn get(` with flexible whitespace
        let pattern = format!("pub fn {fn_name}");
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&pattern) {
                // Verify it's followed by `(` or whitespace-then-`(` to avoid
                // matching e.g. `pub fn get_user` when looking for `get`.
                let rest = &trimmed[pattern.len()..];
                let rest_trimmed = rest.trim_start();
                if rest_trimmed.starts_with('(') {
                    found.push((fn_name, variant));
                    break;
                }
            }
        }
    }

    found
}

/// Scan a Rust source file for a `#[route(path = "...")]` attribute and return
/// the override path segment if present.
///
/// Uses simple string scanning (no `syn` dependency). Matches patterns like:
/// - `#[route(path = "ogimage.png")]`
/// - `#[route( path = "ogimage.png" )]`
///
/// Returns `Some("ogimage.png")` if found, `None` otherwise.
fn scan_route_attribute(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("#[route(") {
            continue;
        }
        // Extract content between `#[route(` and `)]`
        let after_open = trimmed.strip_prefix("#[route(")?;
        let inner = after_open.strip_suffix(")]")?;
        // Look for `path = "..."` within the attribute arguments
        for arg in inner.split(',') {
            let arg = arg.trim();
            if let Some(rest) = arg.strip_prefix("path") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim();
                    if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 {
                        let value = &rest[1..rest.len() - 1];
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn sanitize_mod(name: &str) -> String {
    // With the new naming convention, all filenames are valid Rust identifiers:
    // - `_param` prefixed names are dynamic segments (already valid identifiers)
    // - `all` is the catch-all filename (already a valid identifier)
    // - No more `:param` or `*` filenames
    name.replace('.', "_")
}

/// Convert a raw filesystem prefix (e.g. `/_postId/edit`) to a route prefix
/// (e.g. `/:postId/edit`). Each segment is mapped through `name_to_route_segment`.
fn prefix_to_route_path(prefix: &str) -> String {
    let parts: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| name_to_route_segment(s))
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
        .map(|s| name_to_route_segment(s))
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
