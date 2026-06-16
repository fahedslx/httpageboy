use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct RouteDoc {
  method: String,
  path: String,
  handler: String,
  summary: String,
  headers: Vec<String>,
  permission: Option<String>,
  request_body: Option<String>,
  responses: Vec<(String, String)>,
}

fn quote(value: &str) -> String {
  format!("\"{}\"", value.replace('"', "\\\""))
}

fn collect_rs_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
  for entry in fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))? {
    let entry = entry.map_err(|error| format!("failed to read entry in {}: {error}", path.display()))?;
    let path = entry.path();
    if path.is_dir() {
      collect_rs_files(&path, files)?;
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      files.push(path);
    }
  }
  Ok(())
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
  let from = text.find(start)? + start.len();
  let rest = &text[from..];
  let to = rest.find(end)?;
  Some(&rest[..to])
}

fn extract_route_path(statement: &str) -> Option<String> {
  let start = statement.find(".add_route(")? + ".add_route(".len();
  let rest = statement[start..].trim_start();
  let rest = rest.strip_prefix('"')?;
  let end = rest.find('"')?;
  Some(rest[..end].to_string())
}

fn split_csv(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|item| !item.is_empty())
    .map(ToOwned::to_owned)
    .collect()
}

fn humanize_handler(handler: &str) -> String {
  handler
    .trim()
    .trim_matches('&')
    .replace('_', " ")
    .split_whitespace()
    .map(|word| {
      let mut chars = word.chars();
      match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn parse_doc_comments(
  comments: &[String],
  handler: &str,
) -> (
  String,
  Vec<String>,
  Option<String>,
  Option<String>,
  Vec<(String, String)>,
) {
  let mut summary = String::new();
  let mut headers = Vec::new();
  let mut permission = None;
  let mut request_body = None;
  let mut responses = Vec::new();

  for comment in comments {
    let Some((key, value)) = comment.split_once(':') else {
      continue;
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    match key.as_str() {
      "openapi" | "summary" => summary = value.to_string(),
      "auth" | "headers" => headers.extend(split_csv(value)),
      "permission" => permission = Some(value.to_string()),
      "request" | "requestbody" | "body" => request_body = Some(value.to_string()),
      "response" => {
        let mut parts = value.splitn(2, ' ');
        let status = parts.next().unwrap_or("200").trim();
        let description = parts.next().unwrap_or("OK").trim();
        responses.push((status.to_string(), description.to_string()));
      }
      "errors" => {
        for error in split_csv(value) {
          responses.push(("400".to_string(), error));
        }
      }
      _ => {}
    }
  }

  if summary.is_empty() {
    summary = humanize_handler(handler);
  }

  headers.sort();
  headers.dedup();
  (summary, headers, permission, request_body, responses)
}

fn parse_route(statement: &str, comments: &[String]) -> Option<RouteDoc> {
  if !statement.contains(".add_route(") {
    return None;
  }

  let path = extract_route_path(statement)?;
  let method = extract_between(statement, "Rt::", ",")?.trim().to_ascii_lowercase();
  let handler = extract_between(statement, "handler!(", ")")
    .unwrap_or("")
    .trim()
    .to_string();

  let (summary, headers, permission, request_body, responses) = parse_doc_comments(comments, &handler);

  Some(RouteDoc {
    method,
    path,
    handler,
    summary,
    headers,
    permission,
    request_body,
    responses,
  })
}

fn parse_file(path: &Path) -> Result<Vec<RouteDoc>, String> {
  let content = fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
  let mut routes = Vec::new();
  let mut comments = Vec::new();
  let mut statement = String::new();
  let mut statement_comments = Vec::new();
  let mut reading_route = false;

  for line in content.lines() {
    let trimmed = line.trim();

    if reading_route {
      statement.push(' ');
      statement.push_str(trimmed);
      if trimmed.ends_with(");") {
        if let Some(route) = parse_route(&statement, &statement_comments) {
          routes.push(route);
        }
        statement.clear();
        statement_comments.clear();
        reading_route = false;
      }
      continue;
    }

    if let Some(comment) = trimmed
      .strip_prefix("///")
      .or_else(|| trimmed.strip_prefix("//"))
    {
      comments.push(comment.trim().to_string());
      continue;
    }

    if trimmed.contains(".add_route(") {
      statement = trimmed.to_string();
      statement_comments = comments.clone();
      comments.clear();
      if trimmed.ends_with(");") {
        if let Some(route) = parse_route(&statement, &statement_comments) {
          routes.push(route);
        }
        statement.clear();
        statement_comments.clear();
      } else {
        reading_route = true;
      }
      continue;
    }

    if !trimmed.is_empty() {
      comments.clear();
    }
  }

  Ok(routes)
}

fn load_routes(source: &Path) -> Result<Vec<RouteDoc>, String> {
  let mut files = Vec::new();
  collect_rs_files(source, &mut files)?;
  files.sort();

  let mut routes = Vec::new();
  for file in files {
    routes.extend(parse_file(&file)?);
  }
  routes.sort_by(|left, right| left.path.cmp(&right.path).then(left.method.cmp(&right.method)));
  Ok(routes)
}

fn cargo_field(name: &str, fallback: &str) -> String {
  let Ok(content) = fs::read_to_string("Cargo.toml") else {
    return fallback.to_string();
  };
  for line in content.lines() {
    let trimmed = line.trim();
    if let Some(value) = trimmed.strip_prefix(&format!("{name} = ")) {
      return value.trim().trim_matches('"').to_string();
    }
  }
  fallback.to_string()
}

fn path_params(path: &str) -> Vec<String> {
  let mut params = Vec::new();
  let mut rest = path;
  while let Some(start) = rest.find('{') {
    rest = &rest[start + 1..];
    let Some(end) = rest.find('}') else {
      break;
    };
    params.push(rest[..end].to_string());
    rest = &rest[end + 1..];
  }
  params
}

fn emit_openapi(routes: &[RouteDoc]) -> String {
  let title = env::var("OPENAPI_TITLE").unwrap_or_else(|_| cargo_field("name", "httpageboy-api"));
  let version = env::var("OPENAPI_VERSION").unwrap_or_else(|_| cargo_field("version", "0.1.0"));
  let server = env::var("OPENAPI_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1".to_string());

  let mut out = vec![
    "openapi: 3.0.3".to_string(),
    "info:".to_string(),
    format!("  title: {}", quote(&title)),
    format!("  version: {}", quote(&version)),
    "servers:".to_string(),
    format!("  - url: {}", quote(&server)),
    "paths:".to_string(),
  ];

  let mut grouped: BTreeMap<&str, Vec<&RouteDoc>> = BTreeMap::new();
  for route in routes {
    grouped.entry(&route.path).or_default().push(route);
  }

  for (path, routes) in grouped {
    out.push(format!("  {path}:"));
    for route in routes {
      out.push(format!("    {}:", route.method));
      out.push(format!("      summary: {}", quote(&route.summary)));
      out.push(format!("      operationId: {}", quote(&route.handler)));

      if let Some(permission) = &route.permission {
        out.push("      x-permission:".to_string());
        out.push(format!("        name: {}", quote(permission)));
      }

      let mut has_parameters = false;
      let mut seen_headers = BTreeSet::new();
      let params = path_params(path);
      if !params.is_empty() || !route.headers.is_empty() {
        out.push("      parameters:".to_string());
        has_parameters = true;
      }

      for param in params {
        out.push(format!("        - name: {}", quote(&param)));
        out.push("          in: path".to_string());
        out.push("          required: true".to_string());
        out.push("          schema:".to_string());
        out.push("            type: string".to_string());
      }

      for header in &route.headers {
        if seen_headers.insert(header.to_ascii_lowercase()) {
          out.push(format!("        - name: {}", quote(header)));
          out.push("          in: header".to_string());
          out.push("          required: true".to_string());
          out.push("          schema:".to_string());
          out.push("            type: string".to_string());
        }
      }

      if !has_parameters {
        out.push("      parameters: []".to_string());
      }

      if let Some(body) = &route.request_body {
        out.push("      requestBody:".to_string());
        out.push("        required: true".to_string());
        out.push("        content:".to_string());
        out.push("          application/json:".to_string());
        out.push("            schema:".to_string());
        out.push(format!("              type: {}", quote(body)));
      }

      out.push("      responses:".to_string());
      if route.responses.is_empty() {
        out.push("        \"200\":".to_string());
        out.push("          description: \"OK\"".to_string());
      } else {
        for (status, description) in &route.responses {
          out.push(format!("        {}:", quote(status)));
          out.push(format!("          description: {}", quote(description)));
        }
      }
    }
  }

  format!("{}\n", out.join("\n"))
}

fn write_output(path: &Path, output: &str) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
  }
  fs::write(path, output).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn run() -> Result<(), String> {
  let args: Vec<String> = env::args().collect();
  if args.len() < 3 || args.len() > 4 {
    return Err("usage: openapi_from_code <source src dir> <docs openapi.yaml> [public openapi.yaml]".to_string());
  }

  let routes = load_routes(Path::new(&args[1]))?;
  if routes.is_empty() {
    return Err("no server.add_route calls found".to_string());
  }

  let output = emit_openapi(&routes);
  write_output(Path::new(&args[2]), &output)?;
  if let Some(public_path) = args.get(3) {
    write_output(Path::new(public_path), &output)?;
  }
  Ok(())
}

fn main() {
  if let Err(error) = run() {
    eprintln!("{error}");
    std::process::exit(1);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_route_and_doc_comment() {
    let statement = r#"server.add_route("/users/{id}", Rt::GET, handler!(get_user));"#;
    let comments = vec![
      "openapi: Get user".to_string(),
      "auth: user-token, app-id".to_string(),
      "permission: users.read".to_string(),
      "response: 200 User".to_string(),
    ];

    let route = parse_route(statement, &comments).expect("route");

    assert_eq!(route.path, "/users/{id}");
    assert_eq!(route.method, "get");
    assert_eq!(route.handler, "get_user");
    assert_eq!(route.headers, vec!["app-id".to_string(), "user-token".to_string()]);
    assert_eq!(route.permission, Some("users.read".to_string()));
  }

  #[test]
  fn emits_path_params_and_headers() {
    let route = RouteDoc {
      method: "get".to_string(),
      path: "/users/{id}".to_string(),
      handler: "get_user".to_string(),
      summary: "Get user".to_string(),
      headers: vec!["user-token".to_string()],
      permission: None,
      request_body: None,
      responses: Vec::new(),
    };

    let output = emit_openapi(&[route]);

    assert!(output.contains("  /users/{id}:"));
    assert!(output.contains("operationId: \"get_user\""));
    assert!(output.contains("in: path"));
    assert!(output.contains("name: \"user-token\""));
  }
}
