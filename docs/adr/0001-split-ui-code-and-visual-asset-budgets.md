# ADR 0001: Split UI code and visual asset budgets

## Status

Accepted on 2026-08-05 for the v0.1 release candidate.

## Context

QuotaTide originally enforced one 100 KiB gzip ceiling across every file in the
production UI directory. The later story themes added deliberately encoded PNG
and WebP sprites while the executable HTML, CSS, JavaScript, JSON, and SVG
payload stayed below that ceiling. Counting both categories together made the
gate fail at about 6.49 MiB without identifying code growth or visual-asset
growth separately.

## Decision

PERF-01 retains a 100 KiB gzip ceiling for executable code and text resources.
Encoded visual assets have a separate 7 MiB gzip ceiling. Source maps, the
system WebView, and the Tauri runtime remain excluded. CI reports every file's
category and fails when either ceiling is exceeded.

## Consequences

- Story-theme assets remain part of the shipped application and are measured.
- JavaScript or CSS growth cannot consume unused image budget.
- New visual assets have less than 0.6 MiB headroom and require optimization or
  a new measured decision before crossing the limit.
- PERF-01 evidence must report both totals from the exact final candidate.
