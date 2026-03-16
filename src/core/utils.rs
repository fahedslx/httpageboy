use std::path::{Path, PathBuf};

pub fn get_content_type_quick(path: &Path) -> String {
  let extension = path.extension().and_then(|s| s.to_str());

  let content_type: &str = match extension {
    Some("png") => "image/png",
    Some("jpg") | Some("jpeg") => "image/jpeg",
    Some("gif") => "image/gif",
    Some("bmp") => "image/bmp",
    Some("svg") => "image/svg+xml",
    Some("webp") => "image/webp",
    Some("html") => "text/html",
    Some("css") => "text/css",
    Some("js") => "application/javascript",
    Some("json") => "application/json",
    Some("xml") => "application/xml",
    Some("pdf") => "application/pdf",
    Some("doc") | Some("docx") => "application/msword",
    Some("xls") | Some("xlsx") => "application/vnd.ms-excel",
    Some("ppt") | Some("pptx") => "application/vnd.ms-powerpoint",
    Some("zip") => "application/zip",
    Some("rar") => "application/x-rar-compressed",
    Some("txt") => "text/plain",
    Some("csv") => "text/csv",
    Some("mp3") => "audio/mpeg",
    Some("wav") => "audio/wav",
    Some("mp4") => "video/mp4",
    Some("avi") => "video/x-msvideo",
    Some("mov") => "video/quicktime",
    Some("ogg") => "audio/ogg",
    Some("ogv") => "video/ogg",
    Some("oga") => "audio/ogg",
    Some("ico") => "image/x-icon",
    _ => "application/octet-stream",
  };

  content_type.to_string()
}

/// Given a base directory `base` and a request path (`/algo.txt?...`), returns the canonical path of the file if it exists and is under `base`.
pub fn secure_path(base: &Path, req_path: &str) -> Option<PathBuf> {
  let rel = req_path.split('?').next().unwrap_or("");
  let rel = rel.trim_start_matches('/');

  let mut full = base.join(rel);
  if full.is_dir() {
    full = full.join("index.html");
  }

  let canon = full.canonicalize().ok()?;
  let abs_base = base.canonicalize().ok()?;
  if canon.starts_with(&abs_base) {
    Some(canon)
  } else {
    None
  }
}
