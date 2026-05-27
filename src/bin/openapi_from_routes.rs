use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
enum Value {
  Bool(bool),
  Null,
  String(String),
  List(Vec<Value>),
  Map(BTreeMap<String, Value>),
}

fn leading_spaces(line: &str) -> usize {
  line.len() - line.trim_start_matches(' ').len()
}

fn strip_comments(text: &str) -> Vec<String> {
  text
    .lines()
    .map(str::trim_end)
    .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
    .map(ToOwned::to_owned)
    .collect()
}

fn parse_scalar(value: &str) -> Value {
  let value = value.trim();
  match value {
    "true" => Value::Bool(true),
    "false" => Value::Bool(false),
    "null" | "~" => Value::Null,
    _ if (value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')) => {
      Value::String(value[1..value.len() - 1].to_string())
    }
    _ => Value::String(value.to_string()),
  }
}

fn split_key_value(content: &str) -> Result<(String, Option<Value>), String> {
  let Some((key, value)) = content.split_once(':') else {
    return Err(format!("expected key/value pair: {content}"));
  };
  let key = key.trim();
  if key.is_empty() {
    return Err(format!("empty key in: {content}"));
  }
  let value = value.trim();
  if value.is_empty() {
    Ok((key.to_string(), None))
  } else {
    Ok((key.to_string(), Some(parse_scalar(value))))
  }
}

fn parse_block(lines: &[String], index: usize, indent: usize) -> Result<(Value, usize), String> {
  if index >= lines.len() || leading_spaces(&lines[index]) < indent {
    return Ok((Value::Map(BTreeMap::new()), index));
  }
  if lines[index].trim_start().starts_with("- ") {
    parse_list(lines, index, indent)
  } else {
    parse_map(lines, index, indent)
  }
}

fn parse_list(lines: &[String], mut index: usize, indent: usize) -> Result<(Value, usize), String> {
  let mut result = Vec::new();
  while index < lines.len() {
    let line = &lines[index];
    if leading_spaces(line) != indent || !line.trim_start().starts_with("- ") {
      break;
    }

    let content = line.trim_start()[2..].trim();
    index += 1;

    if content.is_empty() {
      let (item, next) = parse_block(lines, index, indent + 2)?;
      result.push(item);
      index = next;
      continue;
    }

    if content.contains(':') {
      let (key, value) = split_key_value(content)?;
      let mut item = BTreeMap::new();
      let value = match value {
        Some(value) => value,
        None => {
          let (nested, next) = parse_block(lines, index, indent + 2)?;
          index = next;
          nested
        }
      };
      item.insert(key, value);

      if index < lines.len()
        && leading_spaces(&lines[index]) == indent + 2
        && !lines[index].trim_start().starts_with("- ")
      {
        let (extra, next) = parse_map(lines, index, indent + 2)?;
        if let Value::Map(extra) = extra {
          item.extend(extra);
        }
        index = next;
      }

      result.push(Value::Map(item));
    } else {
      result.push(parse_scalar(content));
    }
  }
  Ok((Value::List(result), index))
}

fn parse_map(lines: &[String], mut index: usize, indent: usize) -> Result<(Value, usize), String> {
  let mut result = BTreeMap::new();
  while index < lines.len() {
    let line = &lines[index];
    if leading_spaces(line) != indent || line.trim_start().starts_with("- ") {
      break;
    }

    let (key, value) = split_key_value(line.trim())?;
    index += 1;
    let value = match value {
      Some(value) => value,
      None => {
        let (nested, next) = parse_block(lines, index, indent + 2)?;
        index = next;
        nested
      }
    };
    result.insert(key, value);
  }
  Ok((Value::Map(result), index))
}

fn load_routes(path: &Path) -> Result<Value, String> {
  let content = fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
  let lines = strip_comments(&content);
  let (data, index) = parse_block(&lines, 0, 0)?;
  if index != lines.len() {
    return Err("could not parse all routes.yaml content".to_string());
  }
  Ok(data)
}

fn as_map<'a>(value: &'a Value, name: &str) -> Result<&'a BTreeMap<String, Value>, String> {
  match value {
    Value::Map(map) => Ok(map),
    _ => Err(format!("{name} must be a map")),
  }
}

fn as_list<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], String> {
  match value {
    Value::List(list) => Ok(list),
    _ => Err(format!("{name} must be a list")),
  }
}

fn get_string(map: &BTreeMap<String, Value>, key: &str, fallback: &str) -> String {
  match map.get(key) {
    Some(Value::String(value)) => value.clone(),
    Some(Value::Bool(value)) => value.to_string(),
    Some(Value::Null) | None | Some(Value::Map(_)) | Some(Value::List(_)) => fallback.to_string(),
  }
}

fn get_list<'a>(map: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a [Value], String> {
  match map.get(key) {
    Some(value) => as_list(value, key),
    None => Ok(&[]),
  }
}

fn quote(value: &str) -> String {
  format!("\"{}\"", value.replace('"', "\\\""))
}

fn emit_openapi(data: &Value) -> Result<String, String> {
  let root = as_map(data, "root")?;
  let service = as_map(
    root
      .get("service")
      .ok_or_else(|| "missing service section".to_string())?,
    "service",
  )?;
  let auth = as_map(
    root.get("auth").ok_or_else(|| "missing auth section".to_string())?,
    "auth",
  )?;
  let routes = as_list(
    root.get("routes").ok_or_else(|| "missing routes section".to_string())?,
    "routes",
  )?;

  let name = get_string(service, "name", "httpageboy-api");
  let version = get_string(service, "version", "0.1.0");
  let server = get_string(service, "server", "http://127.0.0.1");
  let headers = get_list(auth, "headers")?;

  let mut out = vec![
    "openapi: 3.0.3".to_string(),
    "info:".to_string(),
    format!("  title: {}", quote(&name)),
    format!("  version: {}", quote(&version)),
    "servers:".to_string(),
    format!("  - url: {}", quote(&server)),
    "paths:".to_string(),
  ];

  for route in routes {
    let route = as_map(route, "route")?;
    let method = get_string(route, "method", "").to_lowercase();
    let path = get_string(route, "path", "");
    if method.is_empty() || path.is_empty() {
      return Err("each route needs method and path".to_string());
    }

    out.push(format!("  {path}:"));
    out.push(format!("    {method}:"));
    out.push(format!("      summary: {}", quote(&get_string(route, "summary", ""))));

    let permission = get_string(route, "permission", "");
    if !permission.is_empty() {
      out.push("      x-permission:".to_string());
      out.push(format!("        name: {}", quote(&permission)));
    }

    let route_headers = match route.get("headers") {
      Some(value) => as_list(value, "headers")?,
      None => headers,
    };
    if !route_headers.is_empty() {
      out.push("      parameters:".to_string());
      for header in route_headers {
        let Value::String(header) = header else {
          return Err("headers must contain strings".to_string());
        };
        out.push(format!("        - name: {}", quote(header)));
        out.push("          in: header".to_string());
        out.push("          required: true".to_string());
        out.push("          schema:".to_string());
        out.push("            type: string".to_string());
      }
    }

    let body = get_string(route, "requestBody", "");
    if !body.is_empty() {
      out.push("      requestBody:".to_string());
      out.push("        required: true".to_string());
      out.push("        content:".to_string());
      out.push("          application/json:".to_string());
      out.push("            schema:".to_string());
      out.push(format!("              type: {}", quote(&body)));
    }

    out.push("      responses:".to_string());
    let responses = get_list(route, "responses")?;
    if responses.is_empty() {
      out.push("        \"200\":".to_string());
      out.push("          description: \"OK\"".to_string());
    }
    for response in responses {
      let response = as_map(response, "response")?;
      out.push(format!("        {}:", quote(&get_string(response, "status", "200"))));
      out.push(format!(
        "          description: {}",
        quote(&get_string(response, "description", "OK"))
      ));
    }
  }

  Ok(format!("{}\n", out.join("\n")))
}

fn run() -> Result<(), String> {
  let args: Vec<String> = env::args().collect();
  if args.len() != 3 {
    return Err("usage: openapi_from_routes <source routes.yaml> <target openapi.yaml>".to_string());
  }

  let source = Path::new(&args[1]);
  let target = Path::new(&args[2]);
  let data = load_routes(source)?;
  let output = emit_openapi(&data)?;
  if let Some(parent) = target.parent() {
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
  }
  fs::write(target, output).map_err(|error| format!("failed to write {}: {error}", target.display()))?;
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
  fn emits_openapi_from_minimal_routes() {
    let data = Value::Map(BTreeMap::from([
      (
        "service".to_string(),
        Value::Map(BTreeMap::from([
          ("name".to_string(), Value::String("api-sales".to_string())),
          ("version".to_string(), Value::String("0.1.0".to_string())),
          (
            "server".to_string(),
            Value::String("https://api-sales.eqeqo.pe".to_string()),
          ),
        ])),
      ),
      (
        "auth".to_string(),
        Value::Map(BTreeMap::from([(
          "headers".to_string(),
          Value::List(vec![
            Value::String("user-token".to_string()),
            Value::String("business-id".to_string()),
          ]),
        )])),
      ),
      (
        "routes".to_string(),
        Value::List(vec![Value::Map(BTreeMap::from([
          ("method".to_string(), Value::String("GET".to_string())),
          ("path".to_string(), Value::String("/products".to_string())),
          ("summary".to_string(), Value::String("List products".to_string())),
          ("permission".to_string(), Value::String("products.read".to_string())),
          (
            "responses".to_string(),
            Value::List(vec![Value::Map(BTreeMap::from([
              ("status".to_string(), Value::String("200".to_string())),
              ("description".to_string(), Value::String("Product list".to_string())),
            ]))]),
          ),
        ]))]),
      ),
    ]));

    let output = emit_openapi(&data).expect("openapi output");

    assert!(output.contains("openapi: 3.0.3"));
    assert!(output.contains("title: \"api-sales\""));
    assert!(output.contains("  /products:"));
    assert!(output.contains("    get:"));
    assert!(output.contains("name: \"products.read\""));
    assert!(output.contains("name: \"user-token\""));
    assert!(output.contains("\"200\":"));
  }
}
