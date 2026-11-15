# Team — Structure

Last retro: 2026-11-06

## Agents

| Agent | Model | Does | Doesn't |
| --- | --- | --- | --- |
| scout | haiku | read-only exploration, answers with path:line | edit, cargo add/update |
| implementer | sonnet | one brief → one minimal local branch, fmt/clippy/test green | widen scope |
| reviewer | sonnet | diff vs CLAUDE.md rules and acceptance criteria | edit |

No Rust specialist yet — add one if a retro shows recurring unsafe/async/FFI work.

## Skills

- badetitou-team: `task-prep`, `team-retro` `github-flow`
- other: none

## MCP servers

None. (The user-level `pharo` server is not relevant here.)

## GitHub

- Repo: `badetitou/Structure` (public). Issues migrated from `.claude/issues/`
  on 2026-11-13 (local id → GitHub number in each file's `remote:` field;
  done → closed). No project board yet (the user-level "badetitou-team"
  project belongs to another repo).

## Notes from retros

- 2026-11-06: team created by team-setup on an empty folder.
- 2026-11-06 retro (Gradle support, #1–#3, #8): 3 implementer spawns refused by
  auto mode for cloning/building an outside fixture → CLAUDE.md: the user clones
  fixtures. Recurring "don't report below-target metrics" → CLAUDE.md rule 6.
  JDK 17 location noted in CLAUDE.md.
