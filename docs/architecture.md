# Structure — cible d'architecture

Plan de refonte décidé le 2026-11-16. Complète `plan.md` (POC) ; chaque étape
de la feuille de route est une PR à un seul sujet.

## Principe

```
Sources ──► Graph ──► Lens (quotient) ──► Overlays (highlights) ──► Views
(langages)  (IR)      (layer×domain…)     (diff, trace…)            (matrice, détails…)
            └─────────── engine Rust (pur + natif) ──────────┘     └─── UI (TS) ───┘
```

Chaque flèche est un contrat. Le code **pur** (ir, langages, classify,
highlight) compile en wasm ; le code **natif** (git, indexeurs SCIP, cache,
système de fichiers) n'existe que dans l'engine natif.

## 1. Langages

Constat : la correspondance langage → règles/séparateur existe en trois copies
(`core/src/lang`, `wasm/src/lib.rs`, tests de `classify`) ; le pipeline a des
cas Pharo et un `match` de tag de cache ; `LanguageFrontend` mêle parsing pur
et indexeur natif ; `Symbol` porte `http`/`http_calls`/`rpc_calls` et
`EdgeKind` a `Http`/`Rpc` en dur.

Cible :
- crate pur `structure-lang` : trait `Frontend { language, extensions,
  is_source, parse_sources(files) -> ParseOutput, default_rules,
  module_separator }` + registre, utilisé par `core` et `wasm` ;
- trait natif `Indexer` dans `core`, optionnel par langage (détection,
  commande, aide, tag de cache SCIP) — Pharo n'en a pas ;
- `ParseOutput.stats` : métriques génériques (remplace `metrics.sends`) ;
- ports : `Port { protocol, role: Provides | Requires, key }` sur les symboles,
  trait `Linker` → arêtes `EdgeKind::Remote { protocol }` ; `http_link` et
  `rpc_link` deviennent deux linkers (registre `core::linker`, lancés par le
  pipeline : HTTP en multi-racine seulement, GWT RPC sur une ou plusieurs
  racines). Décisions : le protocole est une chaîne (`http`, `gwt-rpc`) ; la
  clé est une petite enum par forme (`Endpoint {verb, path}`,
  `ServiceMethod {service, method, arity}`) qu'un nouveau protocole réutilise ;
  les services GWT n'ont pas de port `Provides` : le linker trouve les
  implémentations dans la hiérarchie de types du graphe (fusionné), qu'un
  frontend ne voit pas en parsant une racine. Le `Matrix` garde ses champs
  `http_edges`/`rpc_edges` (un par protocole connu) ;
- un dépôt peut contenir plusieurs langages : la détection rend une liste
  (`detect_languages`, ordre du registre ; avec plusieurs marqueurs, les
  langages sans source sont écartés), `multi.rs` fusionne des unités
  `(racine, langage)`. Décisions : une racine mono-langage garde son nom et
  `index.scip` (résultat identique) ; une racine multi-langage donne une
  unité par langage nommée `<dossier> (<langage>)`, avec `index.<langage>.scip`,
  ses sources, ses règles par défaut + le fichier de règles de la racine ;
  l'impact projette les fichiers modifiés sur chaque unité de la racine ; l'UI
  ne compte comme multi-projet (panneaux git indisponibles) que plusieurs
  racines distinctes.

## 2. Couches configurables

Les règles nomment déjà les couches par chaîne ; seuls la liste et l'ordre
sont figés (enum `Layer` dans `model.rs`, type `Layer` généré dans
`app/ui/src/generated/`).

Cible :
- `Layer` devient `LayerId(String)` ; `Matrix.layers` porte les définitions
  `{ id, label, order }` ; `unknown` reste implicite, toujours en dernier ;
- section `[[layer]]` dans les règles TOML ; les règles par défaut déclarent
  les couches standard ;
- fichier de projet optionnel (`.structure/rules.toml`) qui ajoute, renomme ou
  réordonne ; en multi-projet, fusion par `id` : la première racine
  l'emporte (libellé, ordre), les couches propres aux suivantes sont
  ajoutées après les siennes (`crates/lang/rules/README.md`) ;
- dynamique : `reclassify(paths)` relit les règles et recalcule la matrice
  depuis le graphe en cache, sans réindexer ni reparser (nouvelle génération ;
  les snapshots de commit en cache sont abandonnés) ; l'app sonde
  `rules_stamp(paths)` et recharge quand le fichier de règles change. Édition
  depuis l'UI plus tard.

## 3. Séparation front/back

Constat : cache et orchestration vivaient dans `app/src-tauri/src/commands.rs`
(déplacés dans `structure_core::session` par #71) ; les types sont écrits deux fois (`model.rs`, `api.ts`) ; le `Backend` TS
signale l'indisponibilité en rejetant des promesses.

Cible :
- engine Rust `Session` : possède le cache, expose `open(paths)`, `matrix`,
  `cell_symbols`, `highlight(request)`, `reclassify`, `history`, `diff` ;
  commandes Tauri, CLI et (sous-ensemble) wasm passent par elle ;
- types TS générés depuis Rust — fait (#83) : `ts-rs`, derive derrière la
  feature `ts` de `structure-ir`, fichiers commités dans
  `app/ui/src/generated/`, un test échoue s'ils sont périmés ;
- `Backend` TS avec `capabilities { history, diff, pickFolder, reclassify,
  highlightSources[] }` dont dérivent les `available()` des panneaux, le
  bouton « Reload rules » et la surveillance des règles ; une méthode non
  supportée est absente au lieu de rejeter — fait (#71) ;
  `AppState` découpé en stores (`app/ui/src/stores/`) : analyse, impact (futur
  overlay de highlight), sélection, git, options d'affichage ; la coordination
  entre stores est dans `session.ts` — fait (#71).

## 4. Vues

Acquis : registre de panneaux + dockview ; `ViewContext` (`app/ui/src/workspace/context.ts`,
façade sur les stores et `session.ts`) fourni par contexte Svelte, les vues ne lisent plus les stores (#74, PR 1).
Options d'affichage par instance de panneau (#74, PR 2) : chaque panneau matrice a ses options
(mode, transposé, aligné, liens HTTP/RPC) dans ses paramètres dockview (`params.display`, sauvés
avec la disposition) et son propre `ViewContext` (`forPanel`) ; la barre d'outils agit sur le
panneau matrice actif (focalisé, sinon le dernier focalisé). Un panneau sans paramètres adopte une
fois les valeurs globales (localStorage), qui gardent le dernier choix ; les étiquettes à 45° restent
une préférence globale. Le registre accepte des panneaux multi-instances (`multi`) : « New matrix
view » dans le menu Views.

Cible :
- contrat `ViewContext { lens, overlays, selection, actions }` passé à chaque
  vue, au lieu de lire les stores globaux ; sélection et overlays forment un bus partagé ;
- séparer la *lens* (back : manière de quotienter — layer × domaine, arbre de
  packages, projet × couche) du *rendu* (front : matrice, dense, graphe de
  voisinage, liste) ;
- options d'affichage (dense, transposé, aligné) par instance de panneau
  (paramètres dockview) et non globales : deux matrices côte à côte possibles — fait (PR 2).

## 5. Highlights (overlays)

Les trois étapes de l'impact — résoudre (ancres → symboles), propager (BFS
des appelants ou appelés), agréger (symboles → cellules) — sont dans la crate
pure `structure-highlight` (contrats dans `crates/ir/src/highlight.rs`) ;
`compute_with` (`crates/git/src/impact.rs`) n'y ajoute que git. En JSON,
`Anchor` est étiqueté par `kind` (`Symbol { id }` plutôt que `Symbol(String)`).

Cible, dans le code pur :

```rust
enum Anchor { Symbol(String), Location { file, lines }, Qualified { name, file_hint, line } }
struct Seed { anchor: Anchor, role: Role /* changed, frame(n), … */ }
struct HighlightRequest { source: SourceInfo, seeds: Vec<Seed>,
                          propagate: Propagation /* Callers | Callees | None | Path, max_depth */ }
struct Highlight { source, symbols: [{id, role, distance}], cells: [{id, role, distance}],
                   unresolved: Vec<Anchor>, warnings }
```

- `Impact` est un `Highlight` de source `git-diff` (+ base/head, matrice du
  snapshot) ; « modifié / impacté » est le `role`. `diff_request`
  (`structure-git`) construit la requête (ancres `Location` de rôle `changed`,
  propagation `Callers`) et l'adaptateur unique `Impact::from_highlight`
  (`structure-ir`) en tire le JSON `Impact` inchangé (`changed_*` = rôle
  `changed`, `impacted_*` = le reste avec sa distance). Le `Highlight` lui-même
  ne passe pas encore sur le fil : #70 l'y ajoutera avec son consommateur.
- UI : `app.highlights[]`, overlays nommés — fait (#70) : `impact`
  (`ImpactOverlays`) tient la liste et la sélection, règles pures dans
  `overlays.ts` et `multioverlay.ts`.
- Sélecteur d'overlays : visible seulement quand au moins deux overlays sont
  chargés, dans une barre fine au-dessus de la matrice ; clic = un seul (celui
  par défaut), cmd/ctrl-clic = ajoute ou retire. Chaque overlay a sa couleur
  (tokens `app.css`) ; plusieurs overlays sélectionnés sont dessinés sur la
  matrice du working tree (cellules des snapshots rapportées par id, celles
  qui manquent comptées dans la barre d'état) ; une cellule touchée par
  plusieurs overlays est découpée en bandes, au-delà de trois un style
  « multiple » neutre et l'infobulle les liste — fait (#70).

### Plus tard

- Source stack trace : champ « Highlight depuis une trace » où l'on colle une
  trace ; frames parsées par langage (`Frontend::parse_trace_frame`),
  propagation `Path` ou `None` ; vue Trace.
- Intégration MCP (une session Claude envoie la trace elle-même) seulement
  après la version « coller ».

## Feuille de route

Issue parente : #68 (une sous-issue par étape, une case = une PR).

1. #83 Contrats : génération des types TS depuis Rust.
2. #69 Highlight pur : extraire resolve / propagate / aggregate de
   `compute_with` (sans changement de comportement) ; modèle `Highlight`,
   `Impact` comme source git.
3. #70 UI highlights : `app.highlights[]`, sélecteur, rendu multi-overlay — fait.
4. #71 Engine `Session` : sortir cache et orchestration de `commands.rs` ;
   puis `Backend` à capacités.
5. #72 Couches : `LayerId` (sans changement de comportement) ; `[[layer]]` ;
   fichier de projet ; `reclassify`.
6. #73 Langages : crate `structure-lang` pur (le wasm l'utilise) ; trait
   `Indexer` ; ports et linkers ; dépôts multi-langages.
7. #74 Vues : `ViewContext` ; options par panneau ; lens configurables.
8. #75 Plus tard : source stack trace (coller), vue Trace ; MCP ensuite.
