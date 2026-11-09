
## Brief (implementer)

Goal: a controller method gets the HTTP endpoints declared on the interface/superclass method it
implements, including interfaces that only exist as generated sources under `target/`/`build/`.
Read first: crates/lang-java/src/lib.rs — the file walk skipping `target|build|node_modules` (~659),
`method_endpoints`/`class_prefixes` (#56), supertype resolution (`index.resolve`, ~700-715), how
`Symbol.http` is filled; crates/lang-java/tests/http_endpoints.rs. Real, read-only input:
/Users/benoit.verhaeghe/Documents/WeMagnus/wegf/backend/services/api-compta/target/generated-sources/openapi/server/src/main/java/fr/bl/wemagnus/wegf/compta/web/api/HabilitationsApi.java
and .../src/main/java/fr/bl/wemagnus/wegf/wegfhabilitation/controller/WeGfHabilitationController.java
(`implements HabilitationsApi`, `@Override` methods, same name/arity). wegf has 1253 generated .java files, 150 `*Api.java`.
Design:
1. Inheritance in parsed code: after endpoints are extracted, a method with no own `http` and whose
   type implements/extends a parsed type (resolved with the existing supertype resolution, transitively)
   takes the endpoints of the supertype method with the same name and arity; the supertype's own
   class-level prefix applies (and the implementing class's own prefix, if any, is prepended as Spring does — check
   and document). The interface method keeps its endpoints too, but `link_http` must prefer the
   implementation: when an endpoint is on both an interface method and its implementation, only the
   implementation counts as a handler (avoid `ambiguous`).
2. Generated sources, endpoints only: also walk `target/generated-sources/**` and `build/generated/**`
   *.java under the root, parse them with the same extractor, but do NOT add their symbols, edges or files
   to the Graph/matrix — keep a side table FQN → {method name/arity → endpoints, class prefix} used only for
   step 1 when a supertype resolves to nothing in parsed code (match by FQN from the import/package, else
   by simple name if unique). Skip if the dirs are absent (unchanged behaviour). Watch parse time on wegf
   and report it.
3. If a class implements an unresolved type whose simple name ends in `Api` and nothing was found,
   add one pipeline warning with the count (not per class).
Measure: `cargo build --release -p structure-cli`; `./target/release/structure analyze --no-scip
/Users/benoit.verhaeghe/Documents/WeMagnus/wegf/backend /Users/benoit.verhaeghe/Documents/WeMagnus/wegf/frontend -o <scratch>`.
Before: `http: 1419/2043 linked, 1 unknown path, 10 ambiguous`. Report after, the unmatched grouped by first
two segments, the ratio excluding `{apiComptaUrl}` casino calls (target >= 80%, CLAUDE.md rule 6), and the
wall time before/after. Never write in, build, or run tools in the wegf tree.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace with
STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures (goldens unchanged — jpetstore-6
has target/ and build/: confirm they stay identical; realworld 17/18 and the 19-endpoint test unchanged).
Unit tests: interface mapping inherited (with interface class prefix), generated-dir interface used but its symbols absent
from the graph, implementation preferred over interface in link_http, no generated dir → unchanged.
Rules: no unwrap/expect outside tests; no new deps.
