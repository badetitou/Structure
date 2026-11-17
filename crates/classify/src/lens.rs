//! Lenses: the ways a classified graph is quotiented into matrix cells. Rows
//! are layers in every lens; the columns differ. A lens is chosen per view by
//! the front end; how a view draws the cells is not the lens' business.
//!
//! - `layer-domain` (default): the classification's domains, inferred by the
//!   rules. Its cell ids are `<layer>:<domain>`, unchanged from before lenses.
//! - `layer-package`: the symbol's raw module path as the frontend wrote it
//!   (Java/Pharo package, TS file directory), no inference, nested into a
//!   tree; compares the inferred domains with the real package structure.
//!   Its cell ids are namespaced, `package/<layer>:<path>`.

use crate::classify::{module_path_of, path_classification};
use crate::matrix::prefixed_id;
use anyhow::bail;
use structure_ir::ir::Graph;
use structure_ir::model::{Classification, LayerId, LensInfo};

/// Domain (and tree label) of the symbols without a package or directory.
pub const DEFAULT_PACKAGE: &str = "(default)";

/// A lens of the registry ([`Lens::ALL`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Lens {
    /// Layers × inferred domains: today's matrix.
    #[default]
    LayerDomain,
    /// Layers × raw packages / directories.
    LayerPackage,
}

impl Lens {
    /// Every lens, the default first.
    pub const ALL: [Lens; 2] = [Lens::LayerDomain, Lens::LayerPackage];

    pub fn id(self) -> &'static str {
        match self {
            Lens::LayerDomain => "layer-domain",
            Lens::LayerPackage => "layer-package",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lens::LayerDomain => "Layer × domain",
            Lens::LayerPackage => "Layer × package",
        }
    }

    pub fn info(self) -> LensInfo {
        LensInfo {
            id: self.id().to_string(),
            label: self.label().to_string(),
        }
    }

    /// The lens of id `id`; an unknown id is an error naming it.
    pub fn parse(id: &str) -> anyhow::Result<Lens> {
        match Lens::ALL.into_iter().find(|l| l.id() == id) {
            Some(l) => Ok(l),
            None => bail!("unknown lens {id}"),
        }
    }

    /// The lens of `id`, the default one when `None`.
    pub fn parse_or_default(id: Option<&str>) -> anyhow::Result<Lens> {
        id.map_or(Ok(Lens::default()), Lens::parse)
    }

    /// Prefix of this lens' cell ids (see [`prefixed_id`]): empty for the
    /// default lens, so its ids stay `<layer>:<domain>`.
    pub fn cell_prefix(self) -> &'static str {
        match self {
            Lens::LayerDomain => "",
            Lens::LayerPackage => "package/",
        }
    }

    /// Cell id of a placement in this lens.
    pub fn cell_id(self, layer: &LayerId, column: &str) -> String {
        prefixed_id(self.cell_prefix(), layer, column)
    }

    /// The classification this lens quotients `graph` with, from the rules'
    /// classification `cls` of a single-language graph (`sep`: the language's
    /// module separator). `None` for the default lens: `cls` itself.
    pub fn classification(
        self,
        graph: &Graph,
        cls: &Classification,
        sep: char,
    ) -> Option<Classification> {
        match self {
            Lens::LayerDomain => None,
            Lens::LayerPackage => Some(package_classification(graph, cls, sep, "")),
        }
    }
}

/// The lenses of the registry, as sent to the front end.
pub fn all() -> Vec<LensInfo> {
    Lens::ALL.into_iter().map(Lens::info).collect()
}

/// `cls` with every domain replaced by the symbol's module path (the
/// `layer-package` columns), for the top-level symbols of `graph` whose id
/// starts with `id_prefix` (all of them for `""`); the ids, and the files
/// module paths are read from, are taken without `id_prefix` /
/// `<project>/` (a merged multi-root graph, see `structure-core`), so the
/// result reads as the project's own. Layers are kept.
pub fn package_classification(
    graph: &Graph,
    cls: &Classification,
    sep: char,
    id_prefix: &str,
) -> Classification {
    let project_dir = id_prefix
        .strip_suffix("::")
        .map(|p| format!("{p}/"))
        .unwrap_or_default();
    let members = graph
        .symbols
        .iter()
        .filter(|s| s.container.is_none())
        .filter_map(|s| {
            let local = s.id.strip_prefix(id_prefix)?;
            let p = cls.placements.get(&s.id)?;
            let file = s.file.strip_prefix(&project_dir).unwrap_or(&s.file);
            let path = module_path_of(s.package.as_deref(), file, sep);
            Some((local, path, p.layer.clone()))
        });
    path_classification(members, sep, DEFAULT_PACKAGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use structure_ir::ir::{Language, Range, Symbol, SymbolKind};
    use structure_ir::model::{DomainNode, Placement};

    fn sym(id: &str, container: Option<&str>, file: &str, package: Option<&str>) -> Symbol {
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        Symbol {
            id: id.into(),
            kind: SymbolKind::Class,
            name: id.into(),
            container: container.map(Into::into),
            file: file.into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            ports: vec![],
            package: package.map(Into::into),
        }
    }

    fn placed(cls: &mut Classification, id: &str, layer: &str, domain: &str) {
        cls.placements.insert(
            id.into(),
            Placement {
                layer: LayerId::from(layer),
                domain: domain.into(),
            },
        );
    }

    fn graph(symbols: Vec<Symbol>) -> Graph {
        Graph {
            root: ".".into(),
            language: Language::Java,
            symbols,
            edges: vec![],
        }
    }

    fn leaves(nodes: &[DomainNode], out: &mut Vec<(String, String, usize)>) {
        for n in nodes {
            if let Some(d) = &n.domain {
                out.push((n.label.clone(), d.clone(), n.size));
            }
            leaves(&n.children, out);
        }
    }

    #[test]
    fn registry_ids_parse_and_default() {
        let ids: Vec<String> = all().into_iter().map(|l| l.id).collect();
        assert_eq!(ids, ["layer-domain", "layer-package"]);
        assert_eq!(Lens::default(), Lens::LayerDomain);
        assert_eq!(Lens::parse("layer-package").unwrap(), Lens::LayerPackage);
        assert_eq!(Lens::parse_or_default(None).unwrap(), Lens::LayerDomain);
        let e = Lens::parse("nope").unwrap_err();
        assert_eq!(e.to_string(), "unknown lens nope");
    }

    #[test]
    fn cell_ids_keep_the_default_lens_and_namespace_others() {
        let l = LayerId::from("business");
        assert_eq!(Lens::LayerDomain.cell_id(&l, "account"), "business:account");
        assert_eq!(
            Lens::LayerPackage.cell_id(&l, "account"),
            "package/business:account"
        );
    }

    #[test]
    fn packages_become_columns_with_a_nested_tree() {
        let g = graph(vec![
            sym("A", None, "a/A.java", Some("com.shop.account")),
            sym("A#m", Some("A"), "a/A.java", Some("com.shop.account")),
            sym("B", None, "b/B.java", Some("com.shop.billing")),
            sym("C", None, "C.java", Some("com.shop")),
            sym("D", None, "D.java", None),
            sym("U", None, "U.java", Some("x")), // unplaced
        ]);
        let mut cls = Classification::default();
        placed(&mut cls, "A", "business", "account");
        placed(&mut cls, "B", "persistence", "account");
        placed(&mut cls, "C", "business", "shop");
        placed(&mut cls, "D", "entry", "misc");
        assert!(Lens::LayerDomain.classification(&g, &cls, '.').is_none());
        let p = Lens::LayerPackage.classification(&g, &cls, '.').unwrap();
        let dom = |id: &str| {
            let x = &p.placements[id];
            (x.layer.as_str().to_string(), x.domain.clone())
        };
        assert_eq!(dom("A"), ("business".into(), "com.shop.account".into()));
        assert_eq!(dom("B"), ("persistence".into(), "com.shop.billing".into()));
        assert_eq!(dom("C"), ("business".into(), "com.shop".into()));
        assert_eq!(dom("D"), ("entry".into(), DEFAULT_PACKAGE.into()));
        assert_eq!(
            p.placements.len(),
            4,
            "methods and unplaced symbols are out"
        );
        let tree = p.domain_tree.unwrap();
        // The default package first, then `com.shop` collapsed into one group.
        assert_eq!(tree[0].label, DEFAULT_PACKAGE);
        assert_eq!(tree[1].label, "com.shop");
        assert_eq!(tree[1].size, 3);
        let mut l = vec![];
        leaves(&tree, &mut l);
        assert_eq!(
            l,
            [
                ("(default)".into(), "(default)".into(), 1),
                ("shop".into(), "com.shop".into(), 1),
                ("account".into(), "com.shop.account".into(), 1),
                ("billing".into(), "com.shop.billing".into(), 1),
            ]
        );
    }

    #[test]
    fn directories_for_slash_languages_and_project_prefixes() {
        let g = graph(vec![
            sym("web::x", None, "web/src/app/x.ts", None),
            sym("web::y", None, "web/src/y.ts", None),
            sym("api::Z", None, "api/Z.java", Some("z")),
        ]);
        let mut cls = Classification::default();
        placed(&mut cls, "web::x", "presentation", "app");
        placed(&mut cls, "web::y", "presentation", "src");
        placed(&mut cls, "api::Z", "business", "z");
        let p = package_classification(&g, &cls, '/', "web::");
        let ids: Vec<(&str, &str)> = p
            .placements
            .iter()
            .map(|(k, v)| (k.as_str(), v.domain.as_str()))
            .collect();
        assert_eq!(ids, [("x", "src/app"), ("y", "src")]);
    }
}
