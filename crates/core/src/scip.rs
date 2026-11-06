//! SCIP index reader: decodes `index.scip` into a small language-neutral model.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use protobuf::Message;
use scip::types::{self, descriptor::Suffix};

use crate::ir::Range;

#[derive(Debug, Clone, Default)]
pub struct ScipIndex {
    pub documents: Vec<ScipDocument>,
    /// From documents' symbol infos and `external_symbols`.
    pub symbols: HashMap<String, ScipSymbol>,
}

#[derive(Debug, Clone)]
pub struct ScipDocument {
    pub relative_path: String,
    pub language: String,
    pub occurrences: Vec<ScipOccurrence>,
}

#[derive(Debug, Clone)]
pub struct ScipOccurrence {
    pub symbol: String,
    pub range: Range,
    pub is_definition: bool,
    pub enclosing_range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct ScipSymbol {
    pub symbol: String,
    pub display_name: Option<String>,
    pub kind: Option<String>,
    pub relationships: Vec<ScipRelationship>,
}

#[derive(Debug, Clone)]
pub struct ScipRelationship {
    pub symbol: String,
    pub is_implementation: bool,
    pub is_reference: bool,
    pub is_type_definition: bool,
    pub is_definition: bool,
}

impl ScipIndex {
    /// (documents, symbols, occurrences)
    pub fn stats(&self) -> (usize, usize, usize) {
        let occ = self.documents.iter().map(|d| d.occurrences.len()).sum();
        (self.documents.len(), self.symbols.len(), occ)
    }
}

pub fn load(path: &Path) -> anyhow::Result<ScipIndex> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    from_bytes(&bytes).with_context(|| format!("decoding {}", path.display()))
}

pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<ScipIndex> {
    let index = types::Index::parse_from_bytes(bytes).context("invalid SCIP protobuf")?;
    let mut out = ScipIndex::default();
    for doc in &index.documents {
        for info in &doc.symbols {
            add_symbol(&mut out.symbols, info);
        }
        out.documents.push(ScipDocument {
            relative_path: doc.relative_path.clone(),
            language: doc.language.clone(),
            occurrences: doc.occurrences.iter().filter_map(convert_occ).collect(),
        });
    }
    for info in &index.external_symbols {
        add_symbol(&mut out.symbols, info);
    }
    Ok(out)
}

fn add_symbol(map: &mut HashMap<String, ScipSymbol>, info: &types::SymbolInformation) {
    if info.symbol.is_empty() {
        return;
    }
    let kind = match info.kind.enum_value() {
        Ok(types::symbol_information::Kind::UnspecifiedKind) | Err(_) => None,
        Ok(k) => Some(format!("{k:?}")),
    };
    let relationships = info
        .relationships
        .iter()
        .map(|r| ScipRelationship {
            symbol: r.symbol.clone(),
            is_implementation: r.is_implementation,
            is_reference: r.is_reference,
            is_type_definition: r.is_type_definition,
            is_definition: r.is_definition,
        })
        .collect();
    let new = ScipSymbol {
        symbol: info.symbol.clone(),
        display_name: (!info.display_name.is_empty()).then(|| info.display_name.clone()),
        kind,
        relationships,
    };
    match map.get_mut(&info.symbol) {
        // Same symbol seen twice: merge relationships, fill gaps.
        Some(old) => {
            old.relationships.extend(new.relationships);
            if old.display_name.is_none() {
                old.display_name = new.display_name;
            }
            if old.kind.is_none() {
                old.kind = new.kind;
            }
        }
        None => {
            map.insert(info.symbol.clone(), new);
        }
    }
}

fn convert_occ(o: &types::Occurrence) -> Option<ScipOccurrence> {
    if o.symbol.is_empty() {
        return None;
    }
    let range = convert_range(&o.range)?;
    let enclosing_range = if o.enclosing_range.is_empty() {
        None
    } else {
        convert_range(&o.enclosing_range)
    };
    Some(ScipOccurrence {
        symbol: o.symbol.clone(),
        range,
        is_definition: o.symbol_roles & 0x1 != 0,
        enclosing_range,
    })
}

/// `[sl, sc, ec]` or `[sl, sc, el, ec]`; `None` if malformed.
fn convert_range(r: &[i32]) -> Option<Range> {
    let u = |i: i32| u32::try_from(i).ok();
    let (sl, sc, el, ec) = match *r {
        [sl, sc, ec] => (u(sl)?, u(sc)?, u(sl)?, u(ec)?),
        [sl, sc, el, ec] => (u(sl)?, u(sc)?, u(el)?, u(ec)?),
        _ => return None,
    };
    if (el, ec) < (sl, sc) {
        return None;
    }
    Some(Range {
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
    })
}

pub fn is_local(symbol: &str) -> bool {
    symbol.starts_with("local ")
}

/// Last descriptor name, without suffix punctuation or backtick escaping.
pub fn simple_name(symbol: &str) -> Option<String> {
    let parsed = descriptors(symbol)?;
    parsed
        .last()
        .map(|d| d.name.clone())
        .filter(|n| !n.is_empty())
}

/// Name of the type descriptor directly enclosing the last descriptor.
pub fn owner_type_name(symbol: &str) -> Option<String> {
    let parsed = descriptors(symbol)?;
    let n = parsed.len();
    if n < 2 {
        return None;
    }
    let owner = &parsed[n - 2];
    (owner.suffix.enum_value() == Ok(Suffix::Type) && !owner.name.is_empty())
        .then(|| owner.name.clone())
}

fn descriptors(symbol: &str) -> Option<Vec<types::Descriptor>> {
    if is_local(symbol) {
        return None;
    }
    scip::symbol::parse_symbol(symbol)
        .ok()
        .map(|s| s.descriptors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Document, Index, Occurrence, Relationship, SymbolInformation};

    const JAVA: &str = "semanticdb maven maven/org.mybatis/jpetstore 6.1.0 org/mybatis/jpetstore/service/AccountService#getAccount().";
    const TS: &str = "scip-typescript npm conduit 0.0.0 src/app/core/auth/`user.service.ts`/UserService#login().";

    fn occ(sym: &str, range: Vec<i32>, roles: i32) -> Occurrence {
        Occurrence {
            symbol: sym.into(),
            range,
            symbol_roles: roles,
            ..Default::default()
        }
    }

    #[test]
    fn roundtrip_model() {
        let mut idx = Index::new();
        let mut d1 = Document::new();
        d1.relative_path = "A.java".into();
        d1.language = "java".into();
        let mut def = occ(JAVA, vec![1, 2, 10], 1);
        def.enclosing_range = vec![1, 0, 3, 1];
        d1.occurrences.push(def);
        d1.occurrences.push(occ("", vec![0, 0, 1], 0));
        d1.occurrences.push(occ("local 1", vec![0, 0, 1, 5], 0));
        d1.occurrences.push(occ(JAVA, vec![1, 2], 0));
        let mut info = SymbolInformation::new();
        info.symbol = JAVA.into();
        info.display_name = "getAccount".into();
        let mut rel = Relationship::new();
        rel.symbol = "x".into();
        rel.is_implementation = true;
        info.relationships.push(rel);
        d1.symbols.push(info);
        let mut d2 = Document::new();
        d2.relative_path = "B.java".into();
        d2.occurrences.push(occ(JAVA, vec![4, 1, 9], 0));
        idx.documents.push(d1);
        idx.documents.push(d2);
        let mut ext = SymbolInformation::new();
        ext.symbol = "ext 1".into();
        idx.external_symbols.push(ext);

        let bytes = idx.write_to_bytes().unwrap();
        let m = from_bytes(&bytes).unwrap();
        assert_eq!(m.stats(), (2, 2, 3));
        let o = &m.documents[0].occurrences[0];
        assert!(o.is_definition);
        assert_eq!(o.range.end_col, 10);
        assert_eq!(o.enclosing_range.unwrap().end_line, 3);
        assert!(!m.documents[1].occurrences[0].is_definition);
        assert!(m.documents[1].occurrences[0].enclosing_range.is_none());
        let s = &m.symbols[JAVA];
        assert_eq!(s.display_name.as_deref(), Some("getAccount"));
        assert!(s.relationships[0].is_implementation);
        assert!(m.symbols.contains_key("ext 1"));
        assert!(from_bytes(&[0xff, 0xff, 0xff]).is_err());
    }

    #[test]
    fn ranges() {
        let r = convert_range(&[3, 4, 9]).unwrap();
        assert_eq!(
            (r.start_line, r.start_col, r.end_line, r.end_col),
            (3, 4, 3, 9)
        );
        let r = convert_range(&[3, 4, 5, 2]).unwrap();
        assert_eq!((r.end_line, r.end_col), (5, 2));
        assert!(convert_range(&[]).is_none());
        assert!(convert_range(&[1, 2]).is_none());
        assert!(convert_range(&[1, 2, 3, 4, 5]).is_none());
        assert!(convert_range(&[-1, 2, 3]).is_none());
        assert!(convert_range(&[3, 4, 2, 9]).is_none());
    }

    #[test]
    fn names() {
        assert_eq!(simple_name(JAVA).as_deref(), Some("getAccount"));
        assert_eq!(owner_type_name(JAVA).as_deref(), Some("AccountService"));
        assert_eq!(simple_name(TS).as_deref(), Some("login"));
        assert_eq!(owner_type_name(TS).as_deref(), Some("UserService"));
        let ty = "scip-java maven x 1 a/b/`HttpClient`#";
        assert_eq!(simple_name(ty).as_deref(), Some("HttpClient"));
        assert_eq!(owner_type_name(ty), None);
        let ov = "semanticdb maven x 1 a/B#m(+1).";
        assert_eq!(simple_name(ov).as_deref(), Some("m"));
        assert_eq!(simple_name("local 12"), None);
        assert!(is_local("local 12"));
        assert!(!is_local(JAVA));
    }
}
