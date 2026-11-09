---
id: 63
title: GWT RPC calls link the client to the server implementation
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

Requested by the user after the real test on e-sedit-rh (GWT): `analyze BLGRHGwt BLGRHServer`
gives `http: 0/0` — GWT talks RPC, not URLs. Shape in e-sedit-rh/BLGRHGwt (19 services):
- client `fr.bl.client.grh.ab.remote.IGwtServiceAb extends RemoteService`, its
  `IGwtServiceAbAsync` (same methods + `AsyncCallback` last param), obtained with
  `IGwtServiceAb.Util.getInstance()` / `GWT.create(IGwtServiceAb.class)`;
- server `fr.bl.server.grh.ab.GwtServiceAb extends RemoteServiceServlet implements IGwtServiceAb`.
Client and server live in the same project here, so this must work for one root too,
not only across roots like HTTP (#58).

## Acceptance criteria
- [ ] A call to a method `m` of an `XAsync` interface whose sync twin `X extends RemoteService`
  gives an RPC edge from the caller to `Impl#m` for each class implementing `X` (arity = async arity - 1).
- [ ] New `EdgeKind::Rpc` (or reuse Http with a protocol tag — decide in the brief), shown in the
  matrix like http_edges (#60: drawn, toggle, details), in single- and multi-root analyses.
- [ ] Works without SCIP (receiver typed `XAsync` field/local, or `X.Util.getInstance()` / `GWT.create(X.class)`),
  and with SCIP when available.
- [ ] e-sedit-rh BLGRHGwt: linked/total RPC calls reported, target >= 80%, unmatched listed.
- [ ] Unit tests on a minimal GWT layout.

## Out of scope (later RPC kinds)
- gRPC (generated stubs ↔ `*ImplBase`), Java RMI, JSON-RPC, Feign/HttpExchange clients as callers
  (#56 already skips them as servers).
