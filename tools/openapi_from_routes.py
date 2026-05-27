#!/usr/bin/env python3
"""Generate a minimal OpenAPI file from a small routes.yaml contract.

This is intentionally dependency-free. It supports only the YAML subset used by
the Eqeqo API route contracts: nested maps, lists, and scalar strings.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any


def leading_spaces(line: str) -> int:
  return len(line) - len(line.lstrip(" "))


def strip_comments(text: str) -> list[str]:
  lines: list[str] = []
  for raw in text.splitlines():
    line = raw.rstrip()
    if not line.strip() or line.lstrip().startswith("#"):
      continue
    lines.append(line)
  return lines


def parse_scalar(value: str) -> Any:
  value = value.strip()
  if value in ("true", "false"):
    return value == "true"
  if value in ("null", "~"):
    return None
  if (value.startswith('"') and value.endswith('"')) or (value.startswith("'") and value.endswith("'")):
    return value[1:-1]
  return value


def split_key_value(content: str) -> tuple[str, Any]:
  key, _, value = content.partition(":")
  key = key.strip()
  if not key:
    raise ValueError(f"Invalid empty key in: {content}")
  if value.strip() == "":
    return key, None
  return key, parse_scalar(value)


def parse_block(lines: list[str], index: int, indent: int) -> tuple[Any, int]:
  if index >= len(lines):
    return {}, index

  current = lines[index]
  current_indent = leading_spaces(current)
  if current_indent < indent:
    return {}, index

  stripped = current.strip()
  if stripped.startswith("- "):
    return parse_list(lines, index, indent)
  return parse_map(lines, index, indent)


def parse_list(lines: list[str], index: int, indent: int) -> tuple[list[Any], int]:
  result: list[Any] = []
  while index < len(lines):
    line = lines[index]
    if leading_spaces(line) != indent or not line.strip().startswith("- "):
      break

    content = line.strip()[2:].strip()
    index += 1

    if content == "":
      item, index = parse_block(lines, index, indent + 2)
      result.append(item)
      continue

    if ":" in content:
      key, value = split_key_value(content)
      item: dict[str, Any] = {}
      if value is None:
        value, index = parse_block(lines, index, indent + 2)
      item[key] = value

      if index < len(lines) and leading_spaces(lines[index]) == indent + 2 and not lines[index].strip().startswith("- "):
        extra, index = parse_map(lines, index, indent + 2)
        item.update(extra)
      result.append(item)
      continue

    result.append(parse_scalar(content))
  return result, index


def parse_map(lines: list[str], index: int, indent: int) -> tuple[dict[str, Any], int]:
  result: dict[str, Any] = {}
  while index < len(lines):
    line = lines[index]
    if leading_spaces(line) != indent or line.strip().startswith("- "):
      break

    key, value = split_key_value(line.strip())
    index += 1
    if value is None:
      value, index = parse_block(lines, index, indent + 2)
    result[key] = value
  return result, index


def load_routes(path: Path) -> dict[str, Any]:
  data, index = parse_block(strip_comments(path.read_text(encoding="utf-8")), 0, 0)
  if index != len(strip_comments(path.read_text(encoding="utf-8"))):
    raise ValueError("Could not parse all routes.yaml content")
  if not isinstance(data, dict):
    raise ValueError("routes.yaml root must be a map")
  return data


def quote(value: Any) -> str:
  if value is None:
    return "null"
  text = str(value).replace('"', '\\"')
  return f'"{text}"'


def emit_openapi(data: dict[str, Any]) -> str:
  service = data.get("service", {})
  auth = data.get("auth", {})
  routes = data.get("routes", [])
  if not isinstance(service, dict) or not isinstance(auth, dict) or not isinstance(routes, list):
    raise ValueError("Expected service, auth and routes sections")

  name = service.get("name", "httpageboy-api")
  version = service.get("version", "0.1.0")
  server = service.get("server", "http://127.0.0.1")
  headers = auth.get("headers", [])
  if not isinstance(headers, list):
    raise ValueError("auth.headers must be a list")

  out: list[str] = [
    "openapi: 3.0.3",
    "info:",
    f"  title: {quote(name)}",
    f"  version: {quote(version)}",
    "servers:",
    f"  - url: {quote(server)}",
    "paths:",
  ]

  for route in routes:
    if not isinstance(route, dict):
      raise ValueError("Each route must be a map")
    method = str(route.get("method", "")).lower()
    path = route.get("path")
    if not method or not path:
      raise ValueError("Each route needs method and path")
    summary = route.get("summary", "")
    permission = route.get("permission")
    responses = route.get("responses", [{"status": "200", "description": "OK"}])
    route_headers = route.get("headers", headers)
    if not isinstance(route_headers, list) or not isinstance(responses, list):
      raise ValueError("headers and responses must be lists")

    out.extend([
      f"  {path}:",
      f"    {method}:",
      f"      summary: {quote(summary)}",
    ])
    if permission:
      out.extend([
        "      x-permission:",
        f"        name: {quote(permission)}",
      ])
    if route_headers:
      out.append("      parameters:")
      for header in route_headers:
        out.extend([
          f"        - name: {quote(header)}",
          "          in: header",
          "          required: true",
          "          schema:",
          "            type: string",
        ])
    body = route.get("requestBody")
    if body:
      out.extend([
        "      requestBody:",
        "        required: true",
        "        content:",
        "          application/json:",
        "            schema:",
        f"              type: {quote(body)}",
      ])
    out.append("      responses:")
    for response in responses:
      if not isinstance(response, dict):
        raise ValueError("Each response must be a map")
      status = response.get("status", "200")
      description = response.get("description", "OK")
      out.extend([
        f"        {quote(status)}:",
        f"          description: {quote(description)}",
      ])

  return "\n".join(out) + "\n"


def main() -> None:
  parser = argparse.ArgumentParser(description="Generate minimal OpenAPI from docs/routes.yaml")
  parser.add_argument("source", type=Path)
  parser.add_argument("target", type=Path)
  args = parser.parse_args()

  data = load_routes(args.source)
  args.target.parent.mkdir(parents=True, exist_ok=True)
  args.target.write_text(emit_openapi(data), encoding="utf-8")


if __name__ == "__main__":
  main()
