/// Get MIME type for a file based on extension
/// Returns application/octet-stream as fallback for unknown types
pub fn get_mime_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("");
    
    match extension.to_lowercase().as_str() {
        // Text / markup
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "txt" => "text/plain",
        
        // Data / APIs
        "json" => "application/json",
        "xml" => "application/xml",
        
        // JavaScript / WASM
        "js" => "application/javascript",
        "wasm" => "application/wasm",
        
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        
        // Fonts
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "eot" => "application/vnd.ms-fontobject",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        
        // Media
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        
        // Archives / binaries
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        
        // Fallback
        _ => "application/octet-stream",
    }
}
