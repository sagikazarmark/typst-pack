# Issue tracker: GitHub

Issues and PRDs for this repo live as GitHub issues. Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`.
- **Read an issue**: `gh issue view <number> --comments`, also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments` with appropriate label and state filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`.
- **Apply or remove labels**: `gh issue edit <number> --add-label "..."` or `--remove-label "..."`.
- **Close an issue**: `gh issue close <number> --comment "..."`.

Infer the repo from `git remote -v`; `gh` does this automatically inside the clone.

## Pull requests as a triage surface

**PRs as a request surface: no.** Set this to `yes` if the repo begins treating external PRs as feature requests.

GitHub shares one number space across issues and PRs, so resolve an ambiguous number with `gh pr view <number>` and fall back to `gh issue view <number>`.

## Skill operations

When a skill says to publish to the issue tracker, create a GitHub issue. When it says to fetch a ticket, run `gh issue view <number> --comments`.

## Wayfinding operations

The map is one issue labelled `wayfinder:map`; its tickets are child issues.

- **Map**: create the issue with `gh issue create --label wayfinder:map`.
- **Child ticket**: link an issue to the map with GitHub's sub-issues endpoint. If sub-issues are unavailable, add the child to a task list in the map body and put `Part of #<map>` at the top of the child body. Label it `wayfinder:<type>`, where type is `research`, `prototype`, `grilling`, or `task`.
- **Blocking**: use GitHub's native issue dependencies. Add an edge with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`, where the blocker database id comes from `gh api repos/<owner>/<repo>/issues/<number> --jq .id`. If dependencies are unavailable, put `Blocked by: #<number>, ...` at the top of the child body.
- **Frontier query**: list the map's open children in map order, dropping assigned tickets and tickets with open blockers. The first remaining ticket is the next frontier ticket.
- **Claim**: run `gh issue edit <number> --add-assignee @me` before doing ticket work.
- **Resolve**: post the answer as a comment, close the ticket, then append its linked name and one-line gist to the map's Decisions-so-far section.
