# Structure — cible d'architecture

Plan de refonte décidé le 2026-11-16. Remplace `plan.md` (POC, historique)
pour la structure actuelle ; chaque étape de la feuille de route est une PR à
un seul sujet. État : toutes les étapes sont faites sauf #75 (plus tard) ; les
écarts restants sont notés en place (« reste ») et résumés à la fin.

## Principe

```
Sources ──► Graph ──► Lens (quotient) ──► Overlays (highlights) ──► Views
(langages)  (IR)      (layer×domain…)     (diff, trace…)            (matrice, détails…)
            └─────────── engine Rust (pur + natif) ──────────┘     └─── UI (TS) ───┘
```

Chaque flèche est un contrat. Le code **pur** (ir, langages, classify,
highlight) compile en wasm ; le code **natif** (git, indexeurs SCIP, cache,
système de fichiers) n'existe que dans l'engine natif. La crate `structure-git`
compile aussi en wasm, qui n'en utilise que la lecture d'un diff
(`changes_from_diff`) et l'impact sur un graphe en mémoire.

## 1. Langages

Constat (avant #73) : la correspondance langage → règles/séparateur existait en
trois copies (`core/src/lang`, `wasm/src/lib.rs`, tests de `classify`) ; le
pipeline avait des cas Pharo et un `match` de tag de cache ; `LanguageFrontend`
mêlait parsing pur et indexeur natif ; `Symbol` portait
`http`/`http_calls`/`rpc_calls` et `EdgeKind` avait `Http`/`Rpc` en dur.

Cible — fait (#73) :
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

Constat (avant #72) : les règles nommaient déjà les couches par chaîne ; seuls
la liste et l'ordre étaient figés (une enum `Layer` dans `model.rs`, retirée
depuis).

Cible — fait (#72) :
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

Constat (avant #71 et #83) : cache et orchestration vivaient dans
`app/src-tauri/src/commands.rs` ; les types étaient écrits deux fois
(`model.rs`, `api.ts`) ; le `Backend` TS signalait l'indisponibilité en
rejetant des promesses.

Cible :
- engine Rust `Session` : possède le cache, expose `open(paths)`, `matrix`,
  `cell_symbols`, `highlight(request)`, `reclassify`, `history`, `diff` ;
  commandes Tauri, CLI et (sous-ensemble) wasm passent par elle — fait en
  partie (#71) : `structure_core::session::Session` possède le cache (working
  trees, snapshots de commit, générations) et expose `analyze` (au lieu de
  `open`), `reclassify`, `rules_stamp`, `impact`, `cell_symbols`, `lenses`,
  `matrix`, `impact_cells`, `history`, `diff` ; les commandes Tauri en sont des
  enveloppes minces. Reste : `highlight(request)` (aucune commande ne rend un
  `Highlight`, voir §5) ; la CLI appelle encore `pipeline` directement
  (`analyze_many`, `impact`, `impact_at`) ; le wasm n'utilise pas `Session`
  (il appelle `structure-lang`, `classify` et `git` sur des fichiers en
  mémoire, sans cache) ;
- types TS générés depuis Rust — fait (#83) : `ts-rs`, derive derrière la
  feature `ts` de `structure-ir`, fichiers commités dans
  `app/ui/src/generated/`, un test échoue s'ils sont périmés ;
- `Backend` TS avec `capabilities { history, diff, pickFolder, reclassify,
  highlightSources[] }` dont dérivent les `available()` des panneaux, le
  bouton « Reload rules » et la surveillance des règles ; une méthode non
  supportée est absente au lieu de rejeter — fait (#71 ; `lenses` ajouté par
  #74) ;
  l'ancien `AppState` (`state.svelte.ts`, retiré) découpé en stores
  (`app/ui/src/stores/`) : analyse, impact (liste d'overlays), sélection, git,
  options d'affichage, lenses ; la coordination entre stores est dans
  `session.ts` — fait (#71).

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
Lens choisie par panneau (#74, PR 3) : registre `structure_classify::lens` (`layer-domain`, défaut,
matrice et ids inchangés ; `layer-package`, chemin de module brut — package Java/Pharo, dossier TS —
sans inférence, en arbre comme `DomainNode`, ids `package/<layer>:<chemin>`). Le back construit la
lens à la demande depuis le graphe et la classification et la met en cache avec l'analyse
(`Analysis::lens`) ; `Session::matrix(paths, head, generation, lens)`, `cell_symbols` prend la lens,
`impact_cells` recalcule les cellules d'un impact depuis ses symboles (étape d'agrégation avec la
correspondance de la lens, `aggregate_by`) : c'est le back qui fait la correspondance, l'UI ne
reçoit pas de table symbole → cellule. Côté UI, la lens est une option d'affichage du panneau
(`params.display.lens`, défaut `layer-domain`, non reprise des préférences globales), choisie dans la
barre d'outils pour le panneau actif (sélecteur visible seulement si le back en offre plusieurs :
`capabilities.lenses`, `Backend.lenses`) ; `stores/lenses.svelte.ts` met en cache matrices et
cellules d'impact par lens, et le `ViewContext` du panneau expose matrice et overlays dans sa lens.
La sélection reste partagée mais retient sa lens : un panneau d'une autre lens ne la dessine pas
(« not in this view ») et ne la synchronise pas.

Cible :
- contrat `ViewContext { lens, overlays, selection, actions }` passé à chaque
  vue, au lieu de lire les stores globaux ; sélection et overlays forment un bus partagé —
  fait (PR 1) ;
- séparer la *lens* (back : manière de quotienter — layer × domaine, arbre de
  packages, projet × couche) du *rendu* (front : matrice, dense, graphe de
  voisinage, liste) — fait pour layer × domaine et layer × package (PR 3) ; projet × couche
  reste à ajouter au registre ;
- options d'affichage (dense, transposé, aligné) par instance de panneau
  (paramètres dockview) et non globales : deux matrices côte à côte possibles — fait (PR 2).

## 5. Highlights (overlays)

Les trois étapes de l'impact — résoudre (ancres → symboles), propager (BFS
des appelants ou appelés), agréger (symboles → cellules) — sont dans la crate
pure `structure-highlight` (contrats dans `crates/ir/src/highlight.rs`) ;
`compute_with` (`crates/git/src/impact.rs`) n'y ajoute que git. En JSON,
`Anchor` est étiqueté par `kind` (`Symbol { id }` plutôt que `Symbol(String)`).

Cible, dans le code pur — fait (#69), sauf la propagation `Path` (avec #75) ;
les rôles sont `changed` et `reached` :

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
  ne passe toujours pas sur le fil : #70 a fait les overlays côté UI sur le
  type `Impact` (et `LensCells` pour une autre lens) ; envoyer un `Highlight`
  (`Session::highlight(request)`) est reporté à son premier consommateur, la
  source stack trace (#75).
- UI : liste d'overlays nommés (la cible disait `app.highlights[]`) — fait
  (#70) : le store `impact` (`ImpactOverlays`, `app/ui/src/stores/`) tient la
  liste et la sélection, règles pures dans `overlays.ts` et `multioverlay.ts`.
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

## 6. Modèle externe (Model)

Structure est le premier client de
[`model`](https://github.com/badetitou/Model) (`tag = "v0.1.1"`). Migration
hybride : les frontends Structure restent la source des symboles (ids
pointillés, annotations, supertypes, ports HTTP/GWT, Pharo), mais la lecture
`index.scip` et le lien SCIP sont délégués à `model-scip` / `model-link`.

`crates/core/src/bridge.rs` sème les symboles du `Graph` dans un
`ModelBuilder` (fichier, portée, langue, parent), lance `model_link::link` en
mode `Lift`, puis traduit les relations en `Edge` IR :

- `Calls` → `Call`, `References` → `Uses`, `Implements` → `Implements` ;
- `Inherits`, `Imports` et les relations custom sont ignorés (les frontends
  produisent déjà `Extends`/`Implements`) ;
- les entités externes créées par Model reprennent le nommage `ext:<type>`
  du linker historique ;
- la propagation des appels d'interface vers les implémentations est
  reproduite dans le pont (c'est une analyse dérivée côté Model).

`scip_cache` décode et compose aussi des documents SCIP avec `model-scip`. Le
vieux `structure-scip` reste pour les helpers de symboles, les tests du lecteur
historique et l'API publique ; il n'est plus sur le chemin de `analyze_unit`.

## Feuille de route

Issue parente : #68 (une sous-issue par étape, une case = une PR).

1. #83 Contrats : génération des types TS depuis Rust — fait.
2. #69 Highlight pur : extraire resolve / propagate / aggregate de
   `compute_with` (sans changement de comportement) ; modèle `Highlight`,
   `Impact` comme source git — fait.
3. #70 UI highlights : liste d'overlays, sélecteur, rendu multi-overlay — fait.
4. #71 Engine `Session` : sortir cache et orchestration de `commands.rs` ;
   puis `Backend` à capacités — fait (Tauri seulement, voir les écarts).
5. #72 Couches : `LayerId` (sans changement de comportement) ; `[[layer]]` ;
   fichier de projet ; `reclassify` — fait.
6. #73 Langages : crate `structure-lang` pur (le wasm l'utilise) ; trait
   `Indexer` ; ports et linkers ; dépôts multi-langages — fait.
7. #74 Vues : `ViewContext` ; options par panneau ; lens configurables — fait.
8. #75 Plus tard : source stack trace (coller), vue Trace ; MCP ensuite.

Écarts restants par rapport à la cible :
- `Session` n'a ni `open(paths)` (c'est `analyze`) ni `highlight(request)` ;
  la CLI et le wasm ne passent pas par elle (§3) ;
- le `Highlight` ne passe pas sur le fil, et la propagation `Path` n'existe
  pas (§5, avec #75) ;
- la lens projet × couche n'est pas au registre (§4) ; l'édition des règles
  depuis l'UI reste à faire (§2).
