use std::fs;
use std::path::Path;

fn main() {
    router_library::build::generate_routes();

    // Workaround: the route generator does not emit #[path] attributes for
    // directories whose names start with `:` (param segments).  Rust cannot
    // resolve `mod postId;` to the on-disk `:postId/` directory without an
    // explicit #[path] annotation.  Patch the generated mod.rs accordingly.
    patch_param_dir_mod(Path::new("src/routes/posts/mod.rs"), "postId", ":postId");
}

/// Replace a plain `pub mod <mod_name>;` line with a `#[path]` annotated
/// version pointing at the actual directory (which starts with `:`).
fn patch_param_dir_mod(mod_path: &Path, mod_name: &str, dir_name: &str) {
    if !mod_path.exists() {
        return;
    }
    let contents = fs::read_to_string(mod_path).unwrap();
    let plain = format!("pub mod {mod_name};\n");
    let patched = format!(
        "#[path = \"./{dir_name}/mod.rs\"]\n#[allow(non_snake_case)]\npub mod {mod_name};\n"
    );

    // Only patch the bare declaration (avoid double-patching the one that
    // already has a #[path] from a same-level file).
    if contents.contains(&plain) && !contents.contains(&patched) {
        let new_contents = contents.replacen(&plain, &patched, 1);
        fs::write(mod_path, new_contents).unwrap();
    }
}
