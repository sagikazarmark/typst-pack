# Domain Docs

How engineering skills should consume this repo's domain documentation.

## Before exploring

- Read `CONTEXT.md` at the repo root.
- Read ADRs in `docs/adr/` that touch the area being explored.
- If either location is absent, proceed silently; domain-modeling skills create documentation lazily when decisions resolve.

## Layout

This is a single-context repository: its glossary is `CONTEXT.md`, and its architectural decisions live in `docs/adr/`.

## Vocabulary

Use the glossary's terms in issue titles, designs, tests, and code. Reconsider a new synonym before adding it; a genuinely missing concept should be resolved through domain modeling.

Flag any conflict with an existing ADR explicitly instead of silently overriding it.
