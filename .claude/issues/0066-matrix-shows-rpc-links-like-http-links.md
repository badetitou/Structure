---
id: 66
title: Matrix shows RPC links like HTTP links
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

#63 adds `EdgeKind::Rpc` and `Matrix.rpc_edges` (GWT RPC, client → server impl). The UI from #60
only draws `http_edges`.

## Acceptance criteria
- [ ] `rpc_edges` expanded and drawn like http edges, with its own token/dash so HTTP and RPC differ;
  same hover/selection; a toolbar toggle per kind (or one "Remote links" toggle with both — decide in the brief).
- [ ] Details panel section lists RPC calls/called-by like HTTP.
- [ ] A docs/samples sample with rpc_edges (no absolute paths); vitest, svelte-check, build green; checked in browser.
