//! Pont entre le modèle construit par `model` et le graphe IR de Structure.
//!
//! Structure garde ses frontends (ids pointillés, annotations, ports HTTP/GWT)
//! et confie à Model la lecture SCIP et le lien. Concrètement, les symboles du
//! graphe sont semés dans un [`ModelBuilder`] avec leur fichier et leurs
//! portées : `model_link` y rattache les définitions SCIP, puis résout les
//! références en relations. Les relations sont ensuite traduites en arêtes IR,
//! avec le nommage `ext:` et la propagation des appels d'interface attendus par
//! les analyses de Structure.

use std::collections::{HashMap, HashSet};

use model::RelationKind;
use model::core::{EntityId, EntityKind, LanguageId, ModelBuilder, Range as ModelRange};
use model::link::{LinkOptions, Unmapped};
use model::scip::ScipIndex;

use structure_ir::ir::{Edge, EdgeKind, Graph, Language, Range as IrRange, SymbolKind};

use crate::link::LinkStats;
use crate::scip::{owner_type_name, simple_name};

/// Traduit une portée IR en portée Model (même convention : 0-based, fin exclue).
fn mrange(r: IrRange) -> ModelRange {
    ModelRange::new(r.start_line, r.start_col, r.end_line, r.end_col)
}

fn entity_kind(k: SymbolKind) -> EntityKind {
    match k {
        SymbolKind::Class => EntityKind::Class,
        SymbolKind::Interface => EntityKind::Interface,
        SymbolKind::Enum => EntityKind::Enum,
        SymbolKind::Method => EntityKind::Method,
        SymbolKind::Constructor => EntityKind::Constructor,
        SymbolKind::Field => EntityKind::Field,
        SymbolKind::Function => EntityKind::Function,
    }
}

fn is_callable(k: SymbolKind) -> bool {
    matches!(
        k,
        SymbolKind::Method | SymbolKind::Constructor | SymbolKind::Function
    )
}

fn language_id(lang: Language) -> LanguageId {
    LanguageId::new(match lang {
        Language::Java => "java",
        Language::TypeScript => "typescript",
        Language::Pharo => "pharo",
    })
}

fn edge_order(a: &Edge, b: &Edge) -> std::cmp::Ordering {
    (&a.from, &a.to, a.kind.rank()).cmp(&(&b.from, &b.to, b.kind.rank()))
}

/// Nom `ext:<type>` d'une entité créée par Model pour un symbole hors index.
///
/// La clé d'une telle entité est le symbole SCIP complet. On reprend la règle
/// du linker historique : le type propriétaire quand il existe, sinon le nom
/// simple du dernier descripteur.
fn external_id(key: &str) -> Option<String> {
    let name = owner_type_name(key).or_else(|| simple_name(key))?;
    Some(format!("ext:{name}"))
}

/// Cible d'une relation Model : un id Structure, ou un nom `ext:` avec son
/// index IR quand il existe (utile pour la propagation aux implémentations).
struct Target {
    id: String,
    symbol: Option<usize>,
}

/// Remplace le lien SCIP de Structure par celui de Model.
///
/// `graph` vient du frontend Structure : ses ids et ses métadonnées sont
/// conservés. Les seules arêtes ajoutées sont celles que Model déduit de
/// l'index ; elles sont triées et dédupliquées comme auparavant.
pub fn link(graph: Graph, index: &ScipIndex, lang: Language) -> (Graph, LinkStats) {
    let mut graph = graph;
    let mut b = ModelBuilder::new();
    let lang = language_id(lang);

    // Grâce à l'ordre d'insertion, l'index d'une entité Model est aussi celui
    // de `back`, qui associe son id Structure (None pour les externes créés
    // par Model). Les clés `structure …` ne peuvent pas collisionner avec un
    // symbole SCIP et sont remplacées par `rekey` lors du matching.
    let mut back: Vec<Option<usize>> = Vec::with_capacity(graph.symbols.len());
    let mut by_id: HashMap<&str, usize> = HashMap::with_capacity(graph.symbols.len());
    let mut parents: Vec<Option<String>> = Vec::with_capacity(graph.symbols.len());
    for (i, s) in graph.symbols.iter().enumerate() {
        // L'index rend la clé unique même quand deux fichiers déclarent le
        // même id Structure (par exemple une copie d'une classe Java).
        let id = b.entity(
            &format!("structure {i} {}", s.id),
            entity_kind(s.kind),
            &s.name,
        );
        debug_assert_eq!(id.index(), i);
        b.set_source(id, &s.file, mrange(s.range), Some(mrange(s.name_range)));
        b.set_language(id, &lang);
        back.push(Some(i));
        parents.push(s.container.clone());
        by_id.insert(s.id.as_str(), i);
    }
    for (i, parent) in parents.iter().enumerate() {
        if let Some(id) = parent.as_deref().and_then(|p| by_id.get(p)).copied() {
            b.set_parent(
                EntityId::from_index(i as u32),
                Some(EntityId::from_index(id as u32)),
            );
        }
    }

    let opts = LinkOptions {
        unmapped: Unmapped::Lift,
        language: None,
    };
    let stats = model::link::link(&mut b, index, &opts);

    // 1. Traduction des relations Model en arêtes IR.
    let model = b.build();
    let mut new_edges: Vec<Edge> = Vec::new();
    let mut impls_of: HashMap<usize, Vec<usize>> = HashMap::new();
    for rel in model.relations() {
        let from = rel.from();
        let Some(src) = back.get(from.id().index()).copied().flatten() else {
            continue;
        };
        let to = rel.to();
        let target = match back.get(to.id().index()).copied().flatten() {
            Some(dst) => Target {
                id: graph.symbols[dst].id.clone(),
                symbol: Some(dst),
            },
            None if to.external() => Target {
                id: match external_id(to.key()) {
                    Some(id) => id,
                    None => continue,
                },
                symbol: None,
            },
            None => continue,
        };
        let internal = target.symbol.is_some();
        let kind = match rel.kind() {
            RelationKind::Calls => EdgeKind::Call,
            RelationKind::References if !internal && to.key().ends_with(").") => EdgeKind::Call,
            RelationKind::References => EdgeKind::Uses,
            RelationKind::Implements => EdgeKind::Implements,
            // `Inherits` est déjà produit par les frontends, `Imports` et les
            // relations custom n'ont pas d'équivalent dans l'IR.
            _ => continue,
        };
        if graph.symbols[src].id == target.id {
            continue;
        }
        if kind == EdgeKind::Implements
            && let Some(dst) = target.symbol
            && is_callable(graph.symbols[dst].kind)
        {
            impls_of.entry(dst).or_default().push(src);
        }
        new_edges.push(Edge {
            from: graph.symbols[src].id.clone(),
            to: target.id,
            kind,
        });
    }

    // 2. Un appel à une méthode d'interface atteint aussi ses
    // implémentations. `model-link` ne fait pas cette propagation : c'est une
    // analyse dérivée, reproduite ici pour garder le comportement Structure.
    if !impls_of.is_empty() {
        let mut extra = Vec::new();
        for e in new_edges.iter().filter(|e| e.kind == EdgeKind::Call) {
            let Some(&m) = by_id.get(e.to.as_str()) else {
                continue;
            };
            let mut seen = HashSet::from([m]);
            let mut stack = vec![m];
            while let Some(c) = stack.pop() {
                for &i in impls_of.get(&c).into_iter().flatten() {
                    if seen.insert(i) {
                        stack.push(i);
                        if graph.symbols[i].id != e.from {
                            extra.push(Edge {
                                from: e.from.clone(),
                                to: graph.symbols[i].id.clone(),
                                kind: EdgeKind::Call,
                            });
                        }
                    }
                }
            }
        }
        new_edges.extend(extra);
    }

    // 3. Fusion, tri, dédup : même ordre stable que le linker historique.
    graph.edges.extend(new_edges);
    graph.edges.sort_unstable_by(edge_order);
    graph.edges.dedup();

    let (mut calls, mut uses, mut implements, mut ext) = (0usize, 0usize, 0usize, 0usize);
    for e in &graph.edges {
        match e.kind {
            EdgeKind::Call => calls += 1,
            EdgeKind::Uses => uses += 1,
            EdgeKind::Implements => implements += 1,
            EdgeKind::Extends | EdgeKind::Remote { .. } => {}
        }
        if e.to.starts_with("ext:") {
            ext += 1;
        }
    }
    let out = LinkStats {
        mapped_symbols: stats.mapped,
        lifted_symbols: stats.lifted,
        unmapped_definitions: stats.unmapped,
        references: stats.references,
        call_edges: calls,
        uses_edges: uses,
        implements_edges: implements,
        ext_edges: ext,
    };
    (graph, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_names_follow_the_historical_rule() {
        assert_eq!(
            external_id("semanticdb maven . . java/util/List#size().").as_deref(),
            Some("ext:List")
        );
        assert_eq!(
            external_id("semanticdb maven . . java/lang/String#").as_deref(),
            Some("ext:String")
        );
    }
}
