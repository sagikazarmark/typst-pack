# Domain Docs

How engineering skills should consume this repo's domain documentation.

## Before exploring

- Read `CONTEXT.md` at the repo root.
- Read ADRs in `docs/adr/` that touch the area being explored.
- If either location is absent, proceed silently; domain-modeling skills create documentation lazily when decisions resolve.

## Layout

This is a single-context repository: its glossary is `CONTEXT.md`, and its architectural decisions live in `docs/adr/`.

## Vocabulary

Use the glossary's terms in issue titles, designs, tests, and code. The glossary holds domain terms only; naming conventions for code live in ADR-0021.

Flag any conflict with an existing ADR explicitly instead of silently overriding it. When a decision changes, rewrite the ADR that holds it so it stays self-contained (ADR-0021); do not add an amending or superseding ADR. Files in `docs/research/` are historical records, not current guidance.
