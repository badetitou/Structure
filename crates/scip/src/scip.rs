//! SCIP index reader: decodes `index.scip` into a small language-neutral model.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use protobuf::CodedInputStream;
use protobuf::Message;
use rayon::prelude::*;
use scip::types::{self, descriptor::Suffix};

use structure_ir::ir::Range;

/// A SCIP symbol string, interned in its [`ScipIndex`] (see [`ScipIndex::name`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(u32);

#[derive(Debug, Clone, Default)]
pub struct ScipIndex {
    pub documents: Vec<ScipDocument>,
    /// From documents' symbol infos and `external_symbols`.
    pub symbols: HashMap<SymbolId, ScipSymbol>,
    /// Every symbol string once, by id: occurrences repeat a few symbols
    /// many times (vscode: 8.8M occurrences, 1M distinct symbols).
    names: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, SymbolId>,
}

#[derive(Debug, Clone)]
pub struct ScipDocument {
    pub relative_path: String,
    pub language: String,
    pub occurrences: Vec<ScipOccurrence>,
}

#[derive(Debug, Clone)]
pub struct ScipOccurrence {
    pub symbol: SymbolId,
    pub range: Range,
    pub is_definition: bool,
}

#[derive(Debug, Clone)]
pub struct ScipSymbol {
    pub display_name: Option<String>,
    pub kind: Option<String>,
    pub relationships: Vec<ScipRelationship>,
}

#[derive(Debug, Clone)]
pub struct ScipRelationship {
    pub symbol: SymbolId,
    pub is_implementation: bool,
    pub is_reference: bool,
    pub is_type_definition: bool,
    pub is_definition: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScipStats {
    pub documents: usize,
    pub symbols: usize,
    pub occurrences: usize,
    pub definitions: usize,
    pub references: usize,
}

impl ScipIndex {
    /// The symbol string of `id`, which must come from this index.
    pub fn name(&self, id: SymbolId) -> &str {
        &self.names[id.0 as usize]
    }

    /// Id of `name` if the index has seen it.
    pub fn id(&self, name: &str) -> Option<SymbolId> {
        self.ids.get(name).copied()
    }

    /// Id of `name`, added to the index when new.
    pub fn intern(&mut self, name: &str) -> anyhow::Result<SymbolId> {
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = SymbolId(u32::try_from(self.names.len()).context("too many SCIP symbols")?);
        let name: Arc<str> = Arc::from(name);
        self.names.push(name.clone());
        self.ids.insert(name, id);
        Ok(id)
    }

    /// Symbol information of `name`, if any.
    pub fn symbol(&self, name: &str) -> Option<&ScipSymbol> {
        self.symbols.get(&self.id(name)?)
    }

    pub fn stats(&self) -> ScipStats {
        let occurrences: usize = self.documents.iter().map(|d| d.occurrences.len()).sum();
        let definitions = self
            .documents
            .iter()
            .flat_map(|d| &d.occurrences)
            .filter(|o| o.is_definition)
            .count();
        ScipStats {
            documents: self.documents.len(),
            symbols: self.symbols.len(),
            occurrences,
            definitions,
            references: occurrences - definitions,
        }
    }
}

pub fn load(path: &Path) -> anyhow::Result<ScipIndex> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut input = CodedInputStream::new(&mut reader);
    // Documents are decoded in batches, so only a bounded amount of raw
    // protobuf sits in memory next to the model.
    decode(&mut input, BATCH_BYTES).with_context(|| format!("decoding {}", path.display()))
}

pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<ScipIndex> {
    decode(&mut CodedInputStream::from_bytes(bytes), BATCH_BYTES)
}

/// Calls `f` with each still-encoded `Document` of the index at `path`, in
/// file order. External symbols and metadata are skipped.
pub fn for_each_raw_document(
    path: &Path,
    f: &mut dyn FnMut(Vec<u8>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut input = CodedInputStream::new(&mut reader);
    while let Some(tag) = input
        .read_raw_tag_or_eof()
        .context("invalid SCIP protobuf")?
    {
        if tag >> 3 == 2 {
            let len = input.read_raw_varint32().context("invalid SCIP protobuf")?;
            f(input.read_raw_bytes(len).context("invalid SCIP protobuf")?)?;
        } else {
            let wt = protobuf::rt::WireType::new(tag & 7).context("invalid SCIP protobuf")?;
            input.skip_field(wt).context("invalid SCIP protobuf")?;
        }
    }
    Ok(())
}

/// `relative_path` of an encoded `Document`, without decoding the rest.
pub fn raw_document_path(raw: &[u8]) -> anyhow::Result<String> {
    let mut input = CodedInputStream::from_bytes(raw);
    let mut path = String::new();
    while let Some(tag) = input
        .read_raw_tag_or_eof()
        .context("invalid SCIP protobuf")?
    {
        if tag == (1 << 3 | 2) {
            // Last value wins, as in a full decode.
            path = input.read_string().context("invalid SCIP protobuf")?;
        } else {
            let wt = protobuf::rt::WireType::new(tag & 7).context("invalid SCIP protobuf")?;
            input.skip_field(wt).context("invalid SCIP protobuf")?;
        }
    }
    Ok(path)
}

/// Index made of encoded `Document`s (e.g. from [`for_each_raw_document`]),
/// decoded in bounded batches like [`load`].
pub fn from_raw_documents(
    docs: impl IntoIterator<Item = anyhow::Result<Vec<u8>>>,
) -> anyhow::Result<ScipIndex> {
    let mut out = ScipIndex::default();
    let mut batch: Vec<Vec<u8>> = Vec::new();
    let mut batch_bytes = 0usize;
    for raw in docs {
        let raw = raw?;
        batch_bytes += raw.len();
        batch.push(raw);
        if batch_bytes >= BATCH_BYTES {
            decode_batch(&mut batch, &mut out)?;
            batch_bytes = 0;
        }
    }
    decode_batch(&mut batch, &mut out)?;
    Ok(out)
}

/// Streams the top-level `Index` message (`documents` = 2,
/// `external_symbols` = 3; metadata and unknown fields are skipped).
fn decode(input: &mut CodedInputStream, batch_limit: usize) -> anyhow::Result<ScipIndex> {
    let mut out = ScipIndex::default();
    let mut external: Vec<types::SymbolInformation> = Vec::new();
    let mut batch: Vec<Vec<u8>> = Vec::new();
    let mut batch_bytes = 0usize;
    while let Some(tag) = input
        .read_raw_tag_or_eof()
        .context("invalid SCIP protobuf")?
    {
        match tag >> 3 {
            2 => {
                // Raw document bytes are batched, then decoded in parallel
                // (in order); only one batch of raw bytes is ever resident.
                let len = input.read_raw_varint32().context("invalid SCIP protobuf")?;
                let raw = input.read_raw_bytes(len).context("invalid SCIP protobuf")?;
                batch_bytes += raw.len();
                batch.push(raw);
                if batch_bytes >= batch_limit {
                    decode_batch(&mut batch, &mut out)?;
                    batch_bytes = 0;
                }
            }
            3 => external.push(input.read_message().context("invalid SCIP protobuf")?),
            _ => {
                let wt = protobuf::rt::WireType::new(tag & 7).context("invalid SCIP protobuf")?;
                input.skip_field(wt).context("invalid SCIP protobuf")?;
            }
        }
    }
    decode_batch(&mut batch, &mut out)?;
    for info in &external {
        add_symbol(&mut out, info)?;
    }
    Ok(out)
}

/// Raw (still encoded) document bytes per parallel batch; the decoded batch
/// is larger than this.
const BATCH_BYTES: usize = 64 << 20;

/// One document decoded on a worker thread, its symbols deduplicated
/// locally before they are interned (in document order) into the index.
struct DecodedDocument {
    relative_path: String,
    language: String,
    /// Distinct symbols of the occurrences; `occurrences` index into it.
    names: Vec<String>,
    occurrences: Vec<(u32, Range, bool)>,
    infos: Vec<types::SymbolInformation>,
}

fn decode_document(raw: &[u8]) -> anyhow::Result<DecodedDocument> {
    let doc = types::Document::parse_from_bytes(raw).context("invalid SCIP protobuf")?;
    let mut local: HashMap<String, u32> = HashMap::new();
    let mut occurrences = Vec::with_capacity(doc.occurrences.len());
    for o in doc.occurrences {
        let Some((range, is_definition)) = convert_occ(&o) else {
            continue;
        };
        let next = local.len() as u32;
        let i = *local.entry(o.symbol).or_insert(next);
        occurrences.push((i, range, is_definition));
    }
    let mut names = vec![String::new(); local.len()];
    for (name, i) in local {
        names[i as usize] = name;
    }
    Ok(DecodedDocument {
        relative_path: doc.relative_path,
        language: doc.language,
        names,
        occurrences,
        infos: doc.symbols,
    })
}

/// Decodes the batched documents in parallel and appends them to `out` in
/// their original order, then empties the batch.
fn decode_batch(batch: &mut Vec<Vec<u8>>, out: &mut ScipIndex) -> anyhow::Result<()> {
    let decoded: Vec<anyhow::Result<DecodedDocument>> =
        batch.par_iter().map(|raw| decode_document(raw)).collect();
    batch.clear();
    for d in decoded {
        let d = d?;
        for info in &d.infos {
            add_symbol(out, info)?;
        }
        let ids = d
            .names
            .iter()
            .map(|n| out.intern(n))
            .collect::<anyhow::Result<Vec<SymbolId>>>()?;
        let occurrences = d
            .occurrences
            .into_iter()
            .map(|(i, range, is_definition)| ScipOccurrence {
                symbol: ids[i as usize],
                range,
                is_definition,
            })
            .collect();
        out.documents.push(ScipDocument {
            relative_path: d.relative_path,
            language: d.language,
            occurrences,
        });
    }
    Ok(())
}

fn add_symbol(out: &mut ScipIndex, info: &types::SymbolInformation) -> anyhow::Result<()> {
    if info.symbol.is_empty() {
        return Ok(());
    }
    let kind = match info.kind.enum_value() {
        Ok(types::symbol_information::Kind::UnspecifiedKind) | Err(_) => None,
        Ok(k) => Some(format!("{k:?}")),
    };
    let relationships = info
        .relationships
        .iter()
        .map(|r| {
            Ok(ScipRelationship {
                symbol: out.intern(&r.symbol)?,
                is_implementation: r.is_implementation,
                is_reference: r.is_reference,
                is_type_definition: r.is_type_definition,
                is_definition: r.is_definition,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let new = ScipSymbol {
        display_name: (!info.display_name.is_empty()).then(|| info.display_name.clone()),
        kind,
        relationships,
    };
    let id = out.intern(&info.symbol)?;
    match out.symbols.get_mut(&id) {
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
            out.symbols.insert(id, new);
        }
    }
    Ok(())
}

/// Range and definition flag of an occurrence; `None` when it has no symbol
/// or a malformed range. Its enclosing range is not kept: nothing reads it.
fn convert_occ(o: &types::Occurrence) -> Option<(Range, bool)> {
    if o.symbol.is_empty() {
        return None;
    }
    Some((convert_range(&o.range)?, o.symbol_roles & 0x1 != 0))
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

/// The symbol with its last descriptor removed (`a/B#m().` → `a/B#`), or
/// `None` when nothing meaningful is left (local symbol, package only).
pub fn parent_symbol(symbol: &str) -> Option<String> {
    if is_local(symbol) {
        return None;
    }
    let mut parsed = scip::symbol::parse_symbol(symbol).ok()?;
    parsed.descriptors.pop();
    if parsed.descriptors.is_empty() {
        return None;
    }
    Some(scip::symbol::format_symbol(parsed))
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
        assert_eq!(
            m.stats(),
            ScipStats {
                documents: 2,
                symbols: 2,
                occurrences: 3,
                definitions: 1,
                references: 2
            }
        );
        let got = |d: usize| -> Vec<(String, [u32; 4], bool)> {
            m.documents[d]
                .occurrences
                .iter()
                .map(|o| {
                    let r = o.range;
                    (
                        m.name(o.symbol).to_string(),
                        [r.start_line, r.start_col, r.end_line, r.end_col],
                        o.is_definition,
                    )
                })
                .collect()
        };
        // Empty-symbol and malformed (2-element) occurrences are dropped.
        assert_eq!(
            got(0),
            vec![
                (JAVA.to_string(), [1, 2, 1, 10], true),
                ("local 1".to_string(), [0, 0, 1, 5], false)
            ]
        );
        assert_eq!(got(1), vec![(JAVA.to_string(), [4, 1, 4, 9], false)]);
        let s = m.symbol(JAVA).unwrap();
        assert_eq!(s.display_name.as_deref(), Some("getAccount"));
        assert!(s.relationships[0].is_implementation);
        assert_eq!(m.name(s.relationships[0].symbol), "x");
        assert!(m.symbol("ext 1").is_some());
        // One id per distinct symbol, shared by every occurrence.
        assert_eq!(
            m.documents[0].occurrences[0].symbol,
            m.documents[1].occurrences[0].symbol
        );
    }

    #[test]
    fn raw_documents_roundtrip() {
        let mut idx = Index::new();
        for (p, line) in [("A.java", 1), ("dir/B.java", 4)] {
            let mut d = Document::new();
            d.occurrences.push(occ(JAVA, vec![line, 1, 9], 0));
            // Path written after the occurrences: still found.
            d.relative_path = p.into();
            idx.documents.push(d);
        }
        let mut ext = SymbolInformation::new();
        ext.symbol = "ext 1".into();
        idx.external_symbols.push(ext);
        let file = std::env::temp_dir().join(format!("structure-raw-{}.scip", std::process::id()));
        std::fs::write(&file, idx.write_to_bytes().unwrap()).unwrap();
        let mut raws = Vec::new();
        for_each_raw_document(&file, &mut |r| {
            raws.push(r);
            Ok(())
        })
        .unwrap();
        let _ = std::fs::remove_file(&file);
        let paths: Vec<String> = raws.iter().map(|r| raw_document_path(r).unwrap()).collect();
        assert_eq!(paths, ["A.java", "dir/B.java"]);
        // Only the second document, recomposed: positions kept, externals gone.
        let m = from_raw_documents([Ok(raws[1].clone())]).unwrap();
        assert_eq!(m.documents.len(), 1);
        assert_eq!(m.documents[0].relative_path, "dir/B.java");
        assert_eq!(m.documents[0].occurrences[0].range.start_line, 4);
        assert!(m.symbols.is_empty());
        assert!(raw_document_path(&[0xff, 0xff]).is_err());
    }

    #[test]
    fn invalid_bytes_error() {
        assert!(from_bytes(&[0xff, 0xff, 0xff]).is_err());
    }

    #[test]
    fn symbol_merge() {
        use types::symbol_information::Kind;
        let rel = |s: &str| {
            let mut r = Relationship::new();
            r.symbol = s.into();
            r.is_reference = true;
            r
        };
        let mut doc_info = SymbolInformation::new();
        doc_info.symbol = "s 1".into();
        doc_info.relationships.push(rel("a"));
        // display_name empty and kind unspecified here: filled by external.
        let mut ext = SymbolInformation::new();
        ext.symbol = "s 1".into();
        ext.display_name = "First".into();
        ext.kind = Kind::Class.into();
        ext.relationships.push(rel("b"));
        let mut ext2 = SymbolInformation::new();
        ext2.symbol = "s 1".into();
        ext2.display_name = "Second".into();
        ext2.kind = Kind::Method.into();

        let mut idx = Index::new();
        let mut d = Document::new();
        d.symbols.push(doc_info);
        idx.documents.push(d);
        idx.external_symbols.push(ext);
        idx.external_symbols.push(ext2);
        let m = from_bytes(&idx.write_to_bytes().unwrap()).unwrap();
        assert_eq!(m.symbols.len(), 1);
        let s = m.symbol("s 1").unwrap();
        let rels: Vec<_> = s.relationships.iter().map(|r| m.name(r.symbol)).collect();
        assert_eq!(rels, ["a", "b"]);
        assert_eq!(s.display_name.as_deref(), Some("First"));
        assert_eq!(s.kind.as_deref(), Some("Class"));
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

    fn many_docs_index(n: usize) -> Vec<u8> {
        let mut idx = Index::new();
        for i in 0..n {
            let mut d = Document::new();
            d.relative_path = format!("F{i}.java");
            d.language = "java".into();
            for j in 0..=(i % 3) {
                d.occurrences
                    .push(occ(JAVA, vec![j as i32, 0, 5], (j % 2) as i32));
            }
            let mut info = SymbolInformation::new();
            info.symbol = format!("{JAVA}{i}");
            d.symbols.push(info);
            idx.documents.push(d);
        }
        idx.write_to_bytes().unwrap()
    }

    fn summary(m: &ScipIndex) -> (Vec<(String, usize)>, usize) {
        (
            m.documents
                .iter()
                .map(|d| (d.relative_path.clone(), d.occurrences.len()))
                .collect(),
            m.symbols.len(),
        )
    }

    /// A tiny batch limit forces many batches (and a final partial one); the
    /// result, document order included, matches the single-batch decode.
    #[test]
    fn batched_decode_matches_single_batch() {
        let bytes = many_docs_index(11);
        let one = decode(&mut CodedInputStream::from_bytes(&bytes), usize::MAX).unwrap();
        for limit in [1, 40, 100, 100_000] {
            let m = decode(&mut CodedInputStream::from_bytes(&bytes), limit).unwrap();
            assert_eq!(summary(&m), summary(&one), "limit {limit}");
        }
        let names: Vec<&str> = one
            .documents
            .iter()
            .map(|d| d.relative_path.as_str())
            .collect();
        let want: Vec<String> = (0..11).map(|i| format!("F{i}.java")).collect();
        assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(one.symbols.len(), 11);
    }

    /// A valid frame around an undecodable document is an error, not a panic.
    #[test]
    fn corrupt_document_body_is_an_error() {
        let mut bytes = many_docs_index(2);
        // Field 2 (document), length 3, then a truncated varint / bad tag.
        bytes.extend_from_slice(&[0x12, 0x03, 0x0A, 0xFF, 0xFF]);
        for limit in [1, usize::MAX] {
            let r = decode(&mut CodedInputStream::from_bytes(&bytes), limit);
            assert!(r.is_err(), "limit {limit}");
        }
    }
}
