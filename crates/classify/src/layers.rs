//! Known layers: their ids, labels and order (matrix rows, top to bottom).
//!
//! The layers are declared by the rules (`[[layer]]`, see
//! `RuleSet::layers`); the implicit `unknown` layer is always present and
//! always last.

use structure_ir::model::{LayerDef, LayerId};

/// Layer definitions sorted by `order`, `unknown` last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layers {
    defs: Vec<LayerDef>,
}

impl Layers {
    /// Sorts `defs` by order and appends the implicit `unknown` layer after
    /// them (an `unknown` given in `defs` is dropped).
    pub fn new(defs: impl IntoIterator<Item = LayerDef>) -> Layers {
        let mut defs: Vec<LayerDef> = defs.into_iter().filter(|d| !d.id.is_unknown()).collect();
        defs.sort_by_key(|d| d.order);
        let order = defs.last().map_or(0, |d| d.order.saturating_add(1));
        defs.push(LayerDef {
            id: LayerId::unknown(),
            label: LayerId::UNKNOWN.to_string(),
            order,
        });
        Layers { defs }
    }

    /// All definitions, in order, `unknown` last.
    pub fn defs(&self) -> &[LayerDef] {
        &self.defs
    }

    pub fn get(&self, id: &LayerId) -> Option<&LayerDef> {
        self.defs.iter().find(|d| &d.id == id)
    }

    pub fn contains(&self, id: &LayerId) -> bool {
        self.get(id).is_some()
    }

    /// Row position of `id`; an undefined id sorts after every known layer.
    pub fn rank(&self, id: &LayerId) -> usize {
        self.defs
            .iter()
            .position(|d| &d.id == id)
            .unwrap_or(self.defs.len())
    }

    /// Definition of `id`, or a synthetic one (label = id, after the known
    /// layers) for an undefined id.
    pub fn def_or_default(&self, id: &LayerId) -> LayerDef {
        self.get(id).cloned().unwrap_or_else(|| LayerDef {
            id: id.clone(),
            label: id.to_string(),
            order: self.defs.last().map_or(0, |d| d.order.saturating_add(1)),
        })
    }

    /// Known ids, comma-separated (for error messages).
    pub fn ids(&self) -> String {
        let ids: Vec<&str> = self.defs.iter().map(|d| d.id.as_str()).collect();
        ids.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sorts_and_keeps_unknown_implicit_and_last() {
        let def = |id: &str, order| LayerDef {
            id: id.into(),
            label: id.to_uppercase(),
            order,
        };
        let l = Layers::new([def("b", 5), def("unknown", 0), def("a", 1)]);
        let ids: Vec<&str> = l.defs().iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "unknown"]);
        assert_eq!(l.defs()[2].order, 6);
        assert_eq!(l.rank(&"zzz".into()), 3);
        let z = l.def_or_default(&"zzz".into());
        assert_eq!((z.label.as_str(), z.order), ("zzz", 7));
    }
}
