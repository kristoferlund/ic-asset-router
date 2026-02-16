use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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

/// Generates a route tree from the routes directory and writes it to a file. Also ensures that
/// mod.rs files are created in each directory.
pub fn generate_routes() {
    let routes_dir = Path::new("src/routes");
    let generated_file = Path::new("src/__route_tree.rs");

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

    let mut file = File::create(generated_file).unwrap();
    file.write_all(output.as_bytes()).unwrap();
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

            // Detect middleware.rs — these are not route files.
            if stem == "middleware" {
                children.push("pub mod middleware;\n".to_string());
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
                continue;
            }

            // Detect not_found.rs — custom 404 handler, only in the routes root.
            if stem == "not_found" {
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
