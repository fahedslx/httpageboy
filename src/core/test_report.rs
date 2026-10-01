use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::test_utils::{TestError, TestResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
  Pass,
  Fail,
  Blocked,
  Omit,
  Review,
}

impl fmt::Display for TestStatus {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::Pass => "PASS",
      Self::Fail => "FAIL",
      Self::Blocked => "BLOCKED",
      Self::Omit => "OMIT",
      Self::Review => "REVIEW",
    })
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestReportContext {
  pub id: String,
  pub groups: Vec<(String, String)>,
  pub method: Option<String>,
  pub path: Option<String>,
  pub info: Vec<(String, String)>,
}

impl TestReportContext {
  pub fn new(id: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      ..Self::default()
    }
  }

  pub fn group(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.groups.push((key.into(), value.into()));
    self
  }

  pub fn endpoint(mut self, method: impl Into<String>, path: impl Into<String>) -> Self {
    self.method = Some(method.into());
    self.path = Some(path.into());
    self
  }

  pub fn info(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.info.push((key.into(), value.into()));
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEvaluation {
  pub id: String,
  pub status: TestStatus,
  pub groups: Vec<(String, String)>,
  pub method: Option<String>,
  pub path: Option<String>,
  pub info: Vec<(String, String)>,
  pub expected: Option<String>,
  pub received: Option<String>,
  pub raw: Option<String>,
  pub duration_ms: u128,
}

impl fmt::Display for TestEvaluation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "----- TEST {} -----", self.id)?;
    match (&self.method, &self.path) {
      (Some(method), Some(path)) => writeln!(f, "{} · {} {}", self.status, method, path)?,
      _ => writeln!(f, "{}", self.status)?,
    }

    if !self.groups.is_empty() {
      writeln!(f, "Groups: {}", pairs_line(&self.groups))?;
    }
    if !self.info.is_empty() {
      writeln!(f, "Info: {}", pairs_line(&self.info))?;
    }
    if let Some(expected) = &self.expected {
      writeln!(f, "Expected:\n{}", expected)?;
    }
    if let Some(received) = &self.received {
      writeln!(f, "Received:\n{}", received)?;
    }
    if let Some(raw) = &self.raw {
      writeln!(f, "Rust/Pageboy:\n{}", raw)?;
    }
    writeln!(f, "Duration: {} ms", self.duration_ms)
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestSummary {
  pub total: usize,
  pub pass: usize,
  pub fail: usize,
  pub blocked: usize,
  pub omit: usize,
  pub review: usize,
}

impl TestSummary {
  fn add(&mut self, status: TestStatus) {
    self.total += 1;
    match status {
      TestStatus::Pass => self.pass += 1,
      TestStatus::Fail => self.fail += 1,
      TestStatus::Blocked => self.blocked += 1,
      TestStatus::Omit => self.omit += 1,
      TestStatus::Review => self.review += 1,
    }
  }

  pub fn compact(&self) -> String {
    let mut parts = vec![format!("TOTAL {}", self.total)];
    if self.pass > 0 {
      parts.push(format!("PASS {}", self.pass));
    }
    if self.fail > 0 {
      parts.push(format!("FAIL {}", self.fail));
    }
    if self.blocked > 0 {
      parts.push(format!("BLOCKED {}", self.blocked));
    }
    if self.omit > 0 {
      parts.push(format!("OMIT {}", self.omit));
    }
    if self.review > 0 {
      parts.push(format!("REVIEW {}", self.review));
    }
    parts.join(" · ")
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSummary {
  pub key: String,
  pub value: String,
  pub summary: TestSummary,
}

#[derive(Debug, Default)]
pub struct TestReporter {
  evaluations: Vec<TestEvaluation>,
}

impl TestReporter {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn evaluations(&self) -> &[TestEvaluation] {
    &self.evaluations
  }

  pub fn record_http(
    &mut self,
    context: TestReportContext,
    expected_response: &[u8],
    result: &TestResult<String>,
    duration: Duration,
  ) -> &TestEvaluation {
    self.record_http_with_error_status(
      context,
      expected_response,
      result,
      duration,
      TestStatus::Fail,
    )
  }

  pub fn record_http_with_error_status(
    &mut self,
    context: TestReportContext,
    expected_response: &[u8],
    result: &TestResult<String>,
    duration: Duration,
    error_status: TestStatus,
  ) -> &TestEvaluation {
    let expected = String::from_utf8_lossy(expected_response).to_string();
    let (status, received, raw) = match result {
      Ok(response) => (TestStatus::Pass, Some(response.clone()), None),
      Err(error) => (
        error_status,
        received_from_pageboy_error(&error.message),
        Some(error.message.clone()),
      ),
    };

    self.record(TestEvaluation {
      id: context.id,
      status,
      groups: context.groups,
      method: context.method,
      path: context.path,
      info: context.info,
      expected: Some(expected),
      received,
      raw,
      duration_ms: duration.as_millis(),
    })
  }

  pub fn record_manual(
    &mut self,
    context: TestReportContext,
    status: TestStatus,
    expected: Option<impl Into<String>>,
    received: Option<impl Into<String>>,
    raw: Option<impl Into<String>>,
    duration: Duration,
  ) -> &TestEvaluation {
    self.record(TestEvaluation {
      id: context.id,
      status,
      groups: context.groups,
      method: context.method,
      path: context.path,
      info: context.info,
      expected: expected.map(Into::into),
      received: received.map(Into::into),
      raw: raw.map(Into::into),
      duration_ms: duration.as_millis(),
    })
  }

  fn record(&mut self, evaluation: TestEvaluation) -> &TestEvaluation {
    self.evaluations.push(evaluation);
    self.evaluations.last().expect("just inserted evaluation")
  }

  pub fn summary(&self) -> TestSummary {
    let mut summary = TestSummary::default();
    for evaluation in &self.evaluations {
      summary.add(evaluation.status);
    }
    summary
  }

  pub fn group_summaries(&self, key: &str) -> Vec<GroupSummary> {
    let mut grouped: BTreeMap<String, TestSummary> = BTreeMap::new();

    for evaluation in &self.evaluations {
      for (group_key, group_value) in &evaluation.groups {
        if group_key == key {
          grouped
            .entry(group_value.clone())
            .or_default()
            .add(evaluation.status);
        }
      }
    }

    grouped
      .into_iter()
      .map(|(value, summary)| GroupSummary {
        key: key.to_string(),
        value,
        summary,
      })
      .collect()
  }

  pub fn render_summary(&self, group_keys: &[&str]) -> String {
    let mut output = String::new();
    output.push_str("===== RUN SUMMARY =====\n");
    output.push_str(&self.summary().compact());
    output.push('\n');

    for key in group_keys {
      let groups = self.group_summaries(key);
      if groups.is_empty() {
        continue;
      }
      output.push_str(&format!("\n[{}]\n", key));
      for group in groups {
        output.push_str(&format!("{} · {}\n", group.value, group.summary.compact()));
      }
    }

    output
  }

  pub fn render_text(&self, group_keys: &[&str]) -> String {
    let mut output = String::new();
    for evaluation in &self.evaluations {
      output.push_str(&evaluation.to_string());
      output.push('\n');
    }
    output.push_str(&self.render_summary(group_keys));
    output
  }

  pub fn write_report(
    &self,
    output_dir: impl AsRef<Path>,
    group_keys: &[&str],
  ) -> io::Result<TestReportFiles> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;

    let text = output_dir.join("results.txt");
    let csv = output_dir.join("results.csv");
    let json = output_dir.join("results.json");

    fs::write(&text, self.render_text(group_keys))?;
    fs::write(&csv, self.render_csv())?;
    fs::write(&json, self.render_json())?;

    Ok(TestReportFiles { text, csv, json })
  }

  pub fn fail_if_failed(&self) -> TestResult {
    let summary = self.summary();
    if summary.fail == 0 {
      Ok(())
    } else {
      Err(TestError::new(format!(
        "{} reported test evaluation(s) failed",
        summary.fail
      )))
    }
  }

  fn render_csv(&self) -> String {
    let mut lines = vec![
      [
        "Test ID",
        "Status",
        "Endpoint",
        "Groups",
        "Info",
        "Expected",
        "Received",
        "Rust/Pageboy",
        "Duration ms",
      ]
      .join(","),
    ];

    for evaluation in &self.evaluations {
      let endpoint = match (&evaluation.method, &evaluation.path) {
        (Some(method), Some(path)) => format!("{} {}", method, path),
        _ => String::new(),
      };
      let values = [
        evaluation.id.clone(),
        evaluation.status.to_string(),
        endpoint,
        pairs_line(&evaluation.groups),
        pairs_line(&evaluation.info),
        evaluation.expected.clone().unwrap_or_default(),
        evaluation.received.clone().unwrap_or_default(),
        evaluation.raw.clone().unwrap_or_default(),
        evaluation.duration_ms.to_string(),
      ];
      lines.push(values.iter().map(|value| csv_cell(value)).collect::<Vec<_>>().join(","));
    }

    format!("{}\n", lines.join("\n"))
  }

  fn render_json(&self) -> String {
    let entries = self
      .evaluations
      .iter()
      .map(evaluation_json)
      .collect::<Vec<_>>()
      .join(",\n");

    format!(
      "{{\n  \"summary\": {},\n  \"results\": [\n{}\n  ]\n}}\n",
      summary_json(self.summary()),
      indent(&entries, 4)
    )
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReportFiles {
  pub text: PathBuf,
  pub csv: PathBuf,
  pub json: PathBuf,
}

fn received_from_pageboy_error(message: &str) -> Option<String> {
  let received_marker = "received: ";
  let expected_marker = "\nexpected: ";
  let start = message.find(received_marker)? + received_marker.len();
  let tail = &message[start..];
  let end = tail.rfind(expected_marker)?;
  Some(tail[..end].to_string())
}

fn pairs_line(values: &[(String, String)]) -> String {
  values
    .iter()
    .map(|(key, value)| format!("{}={}", key, value))
    .collect::<Vec<_>>()
    .join(" | ")
}

fn csv_cell(value: &str) -> String {
  if value.contains([',', '"', '\n', '\r']) {
    format!("\"{}\"", value.replace('"', "\"\""))
  } else {
    value.to_string()
  }
}

fn json_escape(value: &str) -> String {
  let mut escaped = String::new();
  for ch in value.chars() {
    match ch {
      '"' => escaped.push_str("\\\""),
      '\\' => escaped.push_str("\\\\"),
      '\n' => escaped.push_str("\\n"),
      '\r' => escaped.push_str("\\r"),
      '\t' => escaped.push_str("\\t"),
      ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
      ch => escaped.push(ch),
    }
  }
  escaped
}

fn json_string(value: &str) -> String {
  format!("\"{}\"", json_escape(value))
}

fn json_pairs(values: &[(String, String)]) -> String {
  let entries = values
    .iter()
    .map(|(key, value)| format!("{}: {}", json_string(key), json_string(value)))
    .collect::<Vec<_>>()
    .join(", ");
  format!("{{{}}}", entries)
}

fn optional_json_string(name: &str, value: &Option<String>) -> Option<String> {
  value
    .as_ref()
    .map(|value| format!("    {}: {}", json_string(name), json_string(value)))
}

fn evaluation_json(evaluation: &TestEvaluation) -> String {
  let mut fields = vec![
    format!("    \"id\": {}", json_string(&evaluation.id)),
    format!("    \"status\": {}", json_string(&evaluation.status.to_string())),
  ];

  if !evaluation.groups.is_empty() {
    fields.push(format!("    \"groups\": {}", json_pairs(&evaluation.groups)));
  }
  if let (Some(method), Some(path)) = (&evaluation.method, &evaluation.path) {
    fields.push(format!("    \"method\": {}", json_string(method)));
    fields.push(format!("    \"path\": {}", json_string(path)));
  }
  if !evaluation.info.is_empty() {
    fields.push(format!("    \"info\": {}", json_pairs(&evaluation.info)));
  }
  if let Some(value) = optional_json_string("expected", &evaluation.expected) {
    fields.push(value);
  }
  if let Some(value) = optional_json_string("received", &evaluation.received) {
    fields.push(value);
  }
  if let Some(value) = optional_json_string("raw", &evaluation.raw) {
    fields.push(value);
  }
  fields.push(format!("    \"duration_ms\": {}", evaluation.duration_ms));

  format!("{{\n{}\n  }}", fields.join(",\n"))
}

fn summary_json(summary: TestSummary) -> String {
  format!(
    "{{\"total\":{},\"pass\":{},\"fail\":{},\"blocked\":{},\"omit\":{},\"review\":{}}}",
    summary.total,
    summary.pass,
    summary.fail,
    summary.blocked,
    summary.omit,
    summary.review
  )
}

fn indent(value: &str, spaces: usize) -> String {
  let prefix = " ".repeat(spaces);
  value
    .lines()
    .map(|line| format!("{}{}", prefix, line))
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn records_pageboy_output_without_rewriting_raw_error() {
    let mut reporter = TestReporter::new();
    let context = TestReportContext::new("CP-N028/stock.read/permission_missing")
      .group("family", "authorization")
      .group("scenario", "permission_missing")
      .endpoint("QUERY", "/inventory/stock")
      .info("business-id", "1")
      .info("permission", "stock.read");

    let raw = "received response did not contain expected response\nreceived: HTTP/1.1 404 Not Found\r\n\r\n404 Not Found\nexpected: HTTP/1.1 403";
    let result: TestResult<String> = Err(TestError::new(raw));
    let evaluation = reporter.record_http(
      context,
      b"HTTP/1.1 403",
      &result,
      Duration::from_millis(84),
    );

    assert_eq!(evaluation.status, TestStatus::Fail);
    assert_eq!(evaluation.expected.as_deref(), Some("HTTP/1.1 403"));
    assert_eq!(
      evaluation.received.as_deref(),
      Some("HTTP/1.1 404 Not Found\r\n\r\n404 Not Found")
    );
    assert_eq!(evaluation.raw.as_deref(), Some(raw));
    assert_eq!(evaluation.duration_ms, 84);

    let rendered = evaluation.to_string();
    assert!(rendered.contains("Expected:\nHTTP/1.1 403"));
    assert!(rendered.contains("Received:\nHTTP/1.1 404 Not Found"));
    assert!(rendered.contains(&format!("Rust/Pageboy:\n{}", raw)));
  }

  #[test]
  fn summarizes_individual_results_by_requested_group() {
    let mut reporter = TestReporter::new();

    reporter.record_manual(
      TestReportContext::new("A").group("scenario", "permission_missing"),
      TestStatus::Pass,
      Some("403"),
      Some("403"),
      None::<String>,
      Duration::from_millis(1),
    );
    reporter.record_manual(
      TestReportContext::new("B").group("scenario", "permission_missing"),
      TestStatus::Fail,
      Some("403"),
      Some("404"),
      Some("assertion failed"),
      Duration::from_millis(2),
    );
    reporter.record_manual(
      TestReportContext::new("C").group("scenario", "cross_business"),
      TestStatus::Blocked,
      Some("404"),
      None::<String>,
      Some("missing fixture"),
      Duration::from_millis(0),
    );

    assert_eq!(
      reporter.summary(),
      TestSummary {
        total: 3,
        pass: 1,
        fail: 1,
        blocked: 1,
        omit: 0,
        review: 0,
      }
    );

    let summary = reporter.render_summary(&["scenario"]);
    assert!(summary.contains("TOTAL 3 · PASS 1 · FAIL 1 · BLOCKED 1"));
    assert!(summary.contains("permission_missing · TOTAL 2 · PASS 1 · FAIL 1"));
    assert!(summary.contains("cross_business · TOTAL 1 · BLOCKED 1"));
  }

  #[test]
  fn writes_text_csv_and_json_reports() {
    let mut reporter = TestReporter::new();
    reporter.record_manual(
      TestReportContext::new("A")
        .group("family", "authorization")
        .endpoint("GET", "/permissions"),
      TestStatus::Pass,
      Some("200"),
      Some("200"),
      None::<String>,
      Duration::from_millis(3),
    );

    let dir = std::env::temp_dir().join(format!(
      "httpageboy-test-report-{}",
      std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let files = reporter.write_report(&dir, &["family"]).unwrap();

    assert!(fs::read_to_string(&files.text).unwrap().contains("----- TEST A -----"));
    assert!(fs::read_to_string(&files.csv).unwrap().contains("Test ID,Status"));
    assert!(fs::read_to_string(&files.json).unwrap().contains("\"results\""));

    let _ = fs::remove_dir_all(&dir);
  }
}
