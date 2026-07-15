# crush-lsp

A Language Server Protocol implementation for [Crush](https://crushlang.org).

```bash
cargo install --path .
# then point your editor's LSP client at the `crush-lsp` binary (stdio transport)
```

## What actually works today

- **Real diagnostics.** Every edit is parsed through [`crush-frontend`](https://github.com/nixpt/crush-ast)
  — the actual Crush compiler frontend — and genuine parse/compiler errors are
  published as LSP diagnostics. Not a stub, not a mock: try opening a file with
  a syntax error and you'll get a real one back.
- **Completions.** A static, hand-written completion dictionary for Crush's
  capability API (`fs.*`, `net.*`, `crypto.*`, `db.*`, `exec.*`) plus general
  language keywords. Fast (no network round-trip), context-aware (triggers on
  `.` and switches by the preceding namespace), and honest about being static
  — not machine-learned, just accurate.
- **Hover.** Calls out to a live LLM via [pipefish](https://github.com/nixpt/seahorse)
  (`PIPEFISH_URL`, defaults to `http://127.0.0.1:11450`) to explain the line
  under the cursor. This is a real network call to a real model — quality
  depends on whatever model pipefish has loaded (defaults to
  `llama3.2-1b.Q8_0`; override with `OLLAMA_MODEL`). A small local model won't
  always know Crush-specific semantics, but the wiring is genuine end-to-end.

## Where this came from

Ported from `exosphere/crates/ai/services/lsp` ("AI Native LSP Server"), which
had real `tower-lsp` protocol scaffolding and this same completion dictionary,
but its "AI/ecosystem integration" layer — `JokerAssistantClient`, `CSSClient`,
`CESClient`, `CDSClient` — was entirely stubbed. Every method on those either
returned `Ok(vec![])` or a hardcoded placeholder string like `"AI-powered
hover information"`. None of it was deleted by mistake or regressed; it was
never real. This project keeps what worked (the protocol layer, the
completions, a genuinely-functional `LlmClient` that was sitting unused
alongside the stubs) and replaces the fake integrations with things that
actually do something:

| Original (exosphere lsp-capsule) | Here |
|---|---|
| `JokerAssistantClient` (stub, `Ok(vec![])`) | deleted |
| `CSSClient::analyze_code` (stub, `Ok(vec![])`) | `crush-frontend::check_source` — real parser |
| `CESClient` / `CDSClient` (stubs, no methods) | deleted |
| `get_hover_info` (hardcoded string) | `LlmClient::explain` — real pipefish call |
| `joker.rs`'s `JokerClient` (real, but unused) | not ported yet — see below |

## Not done yet

- **Code actions.** The original had a `// TODO: Fix AI code actions
  integration` that returned `None` unconditionally; this port also returns
  `None` — honestly, rather than re-adding the TODO. A real version (offer a
  fix via `LlmClient::fix_suggestion` for each diagnostic) is a natural next
  step, needs `execute_command` wired up.
- **`joker.rs`'s `JokerClient`** (routes through Joker MCP's model-routing
  layer rather than hitting pipefish directly) was real and working in the
  original but isn't ported here — `pipefish` alone is simpler and doesn't
  need joker-mcp reachable at LSP runtime. Worth revisiting if multi-provider
  routing matters more than simplicity.
- Structured per-error diagnostic locations on parse *failure* (as opposed to
  successful-parse warnings, which already have real line/col). `check_source`
  only exposes parse failures as a single joined error string; a real fix
  needs `crush-frontend`'s parser to expose structured spans on failure, not
  just success.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE), at your option.
