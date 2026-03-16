
# httpageboy

## Tasks for agents
- No tasks pending.

## Rol
httpageboy is a lightweight HTTP parser/server (sync and async) used as the base for APIs.

## Coding & Commit Rules
- Use **two-space indentation**, no tabs.
- Keep route handlers small and grouped by resource.
- Use imperative commit prefixes: `feat:`, `fix:`, `refactor:`, `docs:`. May there be more than one in a single commit.
- Commit messages must include a prefix (feat/fix/refactor/docs) plus minimal bullet-like summaries of the main changes on subsequent lines. bullets must be plus signs "+ ".
- Avoid adding dependencies when possible.
- Apply KISS practices.

## Security Checklist
- Never commit `.env` or credentials.
- Return correct HTTP status codes on invalid input.
- Protect static file paths via `secure_path`.

## Integration Notes
- Keep compatibility with sync and async runtimes (tokio, async-std, smol).
- Avoid breaking clients that send requests without `Content-Length`.

## Testing Guidelines
- Per-runtime tests live in `tests/test_*`.
- Cover GET/POST/PUT/DELETE with and without `Content-Length`.
