# LSP-1 — Pin + canary the crush-frontend facade

**Status**: Backlog · **Priority**: P2

The survey found crush-lsp's crush-ast surface is deliberately narrow:
`check_source` + `diagnostics::DiagnosticSeverity` (src/lib.rs:185-200) —
HIGH frontend coupling but via just 1 fn + 1 enum. Keep it that way on
purpose: an integration test pinning exactly that surface, plus a CI canary
building against crush-ast main so facade breaks surface early. If the pin
test ever needs widening, that's a design conversation, not a drive-by.

## Done
- [ ] Pin test + canary CI job green
