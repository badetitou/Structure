# Structure — plan du POC

> Plan historique du POC, gardé tel quel. Structure actuelle, feuille de route et
> écarts restants : [`architecture.md`](architecture.md) (et la section « Map » de `CLAUDE.md`).

## Vision

Chaque application est un call graph. On ne l'affiche pas symbole par symbole :
on le **quotiente** en cellules `couche × domaine`, on agrège les arêtes entre
cellules, et on visualise une matrice de carrés qui s'illuminent (BFS avant /
arrière). Objectif : comprendre un logiciel à gros grain, très vite, et servir
la revue de code.

## Ce que le POC doit faire

Une application desktop **Tauri** :

1. **Je sélectionne un logiciel** (dossier Java ou TypeScript).
2. **Il cherche la structure et les arêtes** : tree-sitter (symboles, ranges,
   annotations/décorateurs) + **SCIP** (références exactes issues du
   compilateur), assemblés en call graph, classés, quotientés.
3. **Il construit la visualisation interactive** : matrice couches × domaines,
   survol = BFS avant/arrière.
4. **Pour un diff** (working tree, ou `base..head`), **je visualise l'impact** :
   cellules modifiées + appelants transitifs illuminés.

La visu n'a pas besoin d'être belle : le POC valide le pipeline bout en bout.

Projets de démo :
- **Java : `mybatis/jpetstore-6`** (Maven). Pas du REST : Stripes
  (`*ActionBean` = entrée), Spring DI (`@Service`), MyBatis (`*Mapper` =
  persistance), `domain` = entités/DTO.
- **TypeScript : `gothinkster/angular-realworld-example-app`** (proposé, à
  confirmer) — composants, services `@Injectable`, `HttpClient` = puits.

Hors POC : lien front ↔ back par endpoints, React, OpenAPI, Louvain,
violations de couches, diff de matrices avant/après.

## Outillage (vérifié le 2026-11-06)

| Outil | État |
| --- | --- |
| JDK 21, Maven 3.10 | ✅ |
| Node 26, npm, pnpm | ✅ |
| tauri-cli 2.8 | ✅ |
| scip-java | ❌ — via coursier (`cs launch com.sourcegraph:scip-java_2.13:<ver> -- index`), à installer en T1 |
| scip-typescript | ❌ — via `npx @sourcegraph/scip-typescript index` |

L'app appelle l'indexeur en sous-processus. Si un `index.scip` est déjà
présent, elle le réutilise. Prérequis assumés : le build Maven passe, et
`node_modules` est installé côté TS.

## Architecture

Workspace cargo :

```
crates/core/      lib : ir, scip, java, ts, link, classify, matrix, impact, pipeline
crates/cli/       binaire `structure` (analyze / impact) — tests et debug sans UI
app/src-tauri/    app Tauri 2 : commandes = fines façades sur core::pipeline
app/ui/           front Vite + TS (vanilla + SVG/d3), aucune logique d'analyse
rules/            règles de classification TOML par framework
```

Flux : `detect langage → indexeur SCIP → tree-sitter → link (occurrences SCIP
→ méthode englobante) → Graph → classify → Matrix` ; `git diff → ranges
modifiés → symboles → fermeture des appelants → Impact`.

## Contrats partagés (figés en T0, ne pas modifier sans accord)

1. **IR** (`core::ir`) — `Symbol { id (= symbole SCIP si dispo), kind, name,
   container, file, range, annotations, supertypes }`, `Edge { from, to, kind
   }`, `EdgeKind = Call | Implements | Extends | Uses` ; `Graph`.
2. **Classification** — `symbol_id → (Layer, domain)`, `Layer` ordonné.
3. **Matrix JSON** — `{ layers[], domains[], cells[{id, layer, domain, size,
   symbols[]}], edges[{from, to, weight}] }`.
4. **Impact JSON** — `{ changed_symbols[], changed_cells[],
   impacted_cells[{id, distance}] }`.
5. **API Tauri** (`docs/api.md`) — commandes `analyze(path) → Matrix`,
   `impact(path, base?, head?) → Impact`, `cell_details(id)`, événement
   `analysis-progress { step, pct }`.

Exemples à la main dans `docs/samples/` (matrix, impact) : l'UI se développe
dessus sans attendre le Rust.

## Tâches

Chaque tâche = une branche locale `<slug>`, un agent `implementer` dans son
propre worktree, revue par `reviewer` avant merge dans `main`.
Règles CLAUDE.md : fmt, clippy `-D warnings`, tests.

### Vague 0 — fondations (2 agents)

**T0 · `workspace-contracts`**
- Passage en workspace (`crates/core`, `crates/cli`), types IR / Matrix /
  Impact (serde), `docs/api.md`, `docs/samples/*.json`.
- CLI `structure analyze <path> -o out.json` et `structure impact` en stubs.
- Deps : `serde`, `serde_json`, `clap`, `anyhow`.
- Fini quand : round-trip serde testé sur les samples.

**T1 · `fixtures-indexers`**
- `scripts/fetch-fixtures.sh` : clone les 2 projets à des commits épinglés
  dans `fixtures/` (gitignoré), `mvn` / `npm ci`.
- `scripts/index-fixtures.sh` : installe coursier si absent, produit
  `index.scip` pour les deux ; documente les prérequis.
- Goldens à la main : `tests/golden/{jpetstore,realworld}.toml` (classe →
  couche, domaine) + 3–5 chemins d'appel attendus par projet.
- Fini quand : les deux `index.scip` sont générés ; goldens relus par
  l'utilisateur.

### Vague 1 — 7 agents en parallèle (fichiers disjoints)

**T2 · `scip-reader`** (`core::scip`, dépend de T0, T1)
- Lit `index.scip` (crate `scip` / protobuf) : définitions, occurrences
  (range, rôle), relations `is_implementation`.
- Fini quand : stats correctes sur les 2 index (nb documents, symboles,
  références).

**T3 · `java-treesitter`** (`core::java`, dépend de T0)
- `tree-sitter-java`, parsing parallèle (`rayon`), parcours (`ignore`).
- Classes, interfaces, méthodes, champs avec ranges complets, annotations,
  `extends`/`implements`.

**T4 · `ts-treesitter`** (`core::ts`, dépend de T0)
- `tree-sitter-typescript`, idem : classes, méthodes, fonctions exportées,
  décorateurs (`@Component`, `@Injectable`, `@Controller`…).
- Les templates Angular sont hors POC.

**T5 · `classify-rules`** (`core::classify`, `rules/`, dépend de T0)
- Moteur TOML : annotation/décorateur, supertype, motif de chemin/package,
  suffixe de nom → couche ; domaine = segment de package/dossier ou racine
  du nom.
- Règles `java-stripes-spring-mybatis.toml`, `ts-angular.toml`.
- Signal structurel DTO. Testé sur des `Graph` synthétiques.

**T6 · `quotient-matrix`** (`core::matrix`, dépend de T0)
- `Graph + Classification → Matrix` ; arêtes pondérées ; BFS au niveau
  cellules exposé pour l'UI.

**T7 · `diff-impact`** (`core::impact`, dépend de T0)
- `git diff --unified=0` (CLI git, pas de dépendance) → ranges modifiés →
  symboles touchés (via ranges du `Graph`) → fermeture des appelants →
  `Impact`.
- Testé sur un `Graph` synthétique + un mini dépôt git créé dans le test.
- Limite POC : graphe de la version courante seulement (les symboles
  supprimés sont rattachés à leur fichier).

**T8 · `tauri-app`** (`app/`, dépend de T0)
- Tauri 2 + plugin dialog : choix du dossier, écran de progression, vue
  matrice, survol BFS, panneau de détail d'une cellule, mode « impact »
  (choix base/head, cellules colorées par distance).
- Commandes Tauri **mockées** sur `docs/samples/*.json`.
- Fini quand : `cargo tauri dev` ouvre l'app et tout le parcours marche sur
  les mocks.

### Vague 2 — assemblage

**T9 · `link-callgraph`** (`core::link`, dépend de T2, T3, T4)
- Chaque occurrence SCIP de référence → méthode englobante (ranges
  tree-sitter) → arête `Call`/`Uses` ; `is_implementation` → `Implements`,
  et un appel via interface est propagé aux implémentations.
- Fini quand : sur JPetStore, `AccountActionBean → AccountService →
  AccountMapper` existe ; sur RealWorld, composant → service → `HttpClient`.

**T10 · `pipeline-e2e`** (`core::pipeline`, CLI, Tauri, dépend de tout)
- Détection du langage, lancement de l'indexeur (ou réutilisation de
  `index.scip`), cache par hash, événements de progression.
- Remplace les mocks Tauri par les vraies commandes.
- Tests d'intégration sur les 2 fixtures contre les goldens ; temps affiché.
- Fini quand : démo complète dans l'app (analyse + impact d'un diff réel)
  sur les deux projets.

## Graphe de dépendances

```
T0 ─┬─ T3 ─────┐
    ├─ T4 ─────┼─ T9 ─┐
T1 ─┴─ T2 ─────┘      │
T0 ── T5, T6, T7, T8 ─┴─ T10
```

## Après le POC

1. Lien front ↔ back : routes Spring/Nest → nœuds `Endpoint`, appels HTTP
   front, bande d'endpoints entre deux matrices ; OpenAPI ; orphelins.
2. Templates Angular, puis React (JSX → `renders`, hooks).
3. Revue de code avancée : violations de couches, diff de matrices.
4. Fallback tree-sitter seul quand le build ne passe pas (pas d'index SCIP).
5. Domaines par détection de communautés (Louvain).

## Décisions ouvertes

- Projet de démo TypeScript (RealWorld Angular proposé).
- Granularité des cellules : classes (proposé) ou méthodes.
