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

/// Generates a route tree from the routes directory and writes it to a file. Also ensures that
/// mod.rs files are created in each directory.
pub fn generate_routes() {
    let routes_dir = Path::new("src/routes");
    let generated_file = Path::new("src/__route_tree.rs");

    let mut exports: Vec<MethodExport> = Vec::new();
    process_directory(routes_dir, String::new(), &mut exports);

    // Sort by route path for deterministic output
    exports.sort_by(|a, b| {
        a.route_path
            .cmp(&b.route_path)
            .then(a.method_variant.cmp(&b.method_variant))
    });

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

    output.push_str("        root\n    };\n}\n");

    let mut file = File::create(generated_file).unwrap();
    file.write_all(output.as_bytes()).unwrap();
}

fn process_directory(dir: &Path, prefix: String, exports: &mut Vec<MethodExport>) {
    let mut mod_file = String::new();
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
            process_directory(&path, next_prefix, exports);
            children.push(format!("pub mod {};\n", sanitize_mod(name)));
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let stem = path.file_stem().unwrap().to_str().unwrap();
            if stem == "mod" {
                continue;
            }

            let mod_name = sanitize_mod(stem);
            let route_path = file_to_route_path(&prefix, stem);
            let module_path = file_to_handler_path(&prefix, stem);

            if stem.starts_with(":") || stem == "*" {
                mod_file.push_str(&format!("#[path = \"./{stem}.rs\"]\npub mod {mod_name};\n"));
            } else {
                children.push(format!("pub mod {mod_name};\n"));
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

            for (fn_name, variant) in &methods {
                exports.push(MethodExport {
                    route_path: route_path.clone(),
                    handler_path: format!("{}::{}", module_path, fn_name),
                    method_variant: variant.to_string(),
                });
            }
        }
    }

    if !mod_file.is_empty() || !children.is_empty() {
        let mut contents = mod_file;
        for child in &children {
            contents.push_str(child);
        }
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

fn sanitize_mod(name: &str) -> String {
    match name {
        "*" => "__any".into(),
        s if s.starts_with(":") => s.trim_start_matches(":").into(),
        s => s.replace('.', "_"),
    }
}

fn file_to_route_path(prefix: &str, name: &str) -> String {
    let mut parts = vec![];
    if !prefix.is_empty() {
        parts.push(prefix.to_string());
    }
    parts.push(if name == "index" {
        "".into()
    } else if name == "*" {
        "*".into()
    } else {
        name.to_string()
    });
    parts.join("/").replace("//", "/")
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
