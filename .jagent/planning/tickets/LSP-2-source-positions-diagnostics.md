# LSP-2 — Real source positions in diagnostics (post CRUSH-74)

**Status**: Backlog — GATED on crush-ast CRUSH-74/79 · **Priority**: P2

Today crush-ast drops source locations mid-pipeline (parser emits empty meta),
so LSP diagnostics can't carry real ranges. When crush-ast CRUSH-74 (span
wiring) + CRUSH-79 (casm source_map fix) land, consume them: diagnostics with
accurate line/col ranges, verified against a fixture file with errors at known
positions.

## Done
- [ ] Diagnostics carry real ranges (fixture-verified); gate noted in code until then
