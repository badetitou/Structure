//! GWT RPC call sites: method calls whose receiver is an `XAsync` service
//! (typed field/local/parameter, `X.Util.getInstance()` or `GWT.create(X.class)`).
//!
//! Only the syntactic shape is read here; whether `X` really is a
//! `RemoteService` is decided once the whole project is parsed.

use std::collections::HashMap;

use tree_sitter::Node;

use super::{text, type_name};

/// What a call's receiver says about the service it talks to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receiver {
    /// Declared as `XAsync` (simple name as written).
    Async(String),
    /// The sync interface `X` itself (simple name as written). `strict`: keep
    /// the call only when `X` resolves to a project `RemoteService`.
    Sync { name: String, strict: bool },
}

/// A call on a receiver that may be an RPC service, before resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRpcCall {
    /// Id of the enclosing method or constructor.
    pub caller: String,
    pub recv: Receiver,
    pub method: String,
    pub arity: u32,
}

/// Field or local name -> RPC receiver; `None` for any other type (shadowing).
pub type Env = HashMap<String, Option<Receiver>>;

/// Methods every object has: never RPC calls.
const OBJECT_METHODS: &[&str] = &["toString", "equals", "hashCode", "getClass"];

fn from_type(t: Node, src: &str) -> Option<Receiver> {
    let n = type_name(t, src)?;
    (n.len() > "Async".len() && n.ends_with("Async")).then_some(Receiver::Async(n))
}

/// Receivers of the fields declared directly in a type body.
pub fn field_env(body: Node, src: &str) -> Env {
    let mut env = Env::new();
    let mut cur = body.walk();
    for m in body.named_children(&mut cur) {
        if m.kind() != "field_declaration" {
            continue;
        }
        let recv = m
            .child_by_field_name("type")
            .and_then(|t| from_type(t, src));
        let mut c2 = m.walk();
        for d in m.children_by_field_name("declarator", &mut c2) {
            if let Some(n) = d.child_by_field_name("name") {
                env.insert(text(n, src).to_string(), recv.clone());
            }
        }
    }
    env
}

/// A local: (start byte of the declaration, byte range where it is visible,
/// name, receiver).
type Decl = (usize, (usize, usize), String, Option<Receiver>);

struct Locals {
    decls: Vec<Decl>,
}

/// `n`'s `levels`-th ancestor (or the outermost one reached).
fn up(n: Node, levels: usize) -> Node {
    let mut cur = n;
    for _ in 0..levels {
        match cur.parent() {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

fn param_name<'t>(p: Node<'t>) -> Option<Node<'t>> {
    p.child_by_field_name("name").or_else(|| {
        let mut c = p.walk();
        p.named_children(&mut c)
            .find(|c| c.kind() == "variable_declarator")
            .and_then(|d| d.child_by_field_name("name"))
    })
}

fn collect_locals(n: Node, src: &str, out: &mut Locals) {
    // `scope`: the node whose extent bounds the visibility of the local.
    let mut add = |at: Node, scope: Node, name: Option<Node>, recv: Option<Receiver>| {
        if let Some(nm) = name {
            out.decls.push((
                at.start_byte(),
                (scope.start_byte(), scope.end_byte()),
                text(nm, src).to_string(),
                recv,
            ));
        }
    };
    match n.kind() {
        "local_variable_declaration" => {
            let ty = n.child_by_field_name("type");
            let is_var = ty.is_some_and(|t| text(t, src) == "var");
            let mut c = n.walk();
            for d in n.children_by_field_name("declarator", &mut c) {
                let recv = if is_var {
                    d.child_by_field_name("value")
                        .and_then(|v| receiver(v, src, &[], &[]))
                } else {
                    ty.and_then(|t| from_type(t, src))
                };
                add(d, up(n, 1), d.child_by_field_name("name"), recv);
            }
        }
        "formal_parameter" | "spread_parameter" => {
            let recv = n
                .child_by_field_name("type")
                .and_then(|t| from_type(t, src));
            add(n, up(n, 2), param_name(n), recv);
        }
        "catch_formal_parameter" => add(n, up(n, 1), n.child_by_field_name("name"), None),
        "enhanced_for_statement" => {
            let recv = n
                .child_by_field_name("type")
                .and_then(|t| from_type(t, src));
            add(n, n, n.child_by_field_name("name"), recv);
        }
        "resource" => {
            let recv = n
                .child_by_field_name("type")
                .and_then(|t| from_type(t, src));
            add(n, up(n, 2), n.child_by_field_name("name"), recv);
        }
        "lambda_expression" => {
            if let Some(p) = n.child_by_field_name("parameters") {
                if p.kind() == "identifier" {
                    add(n, n, Some(p), None);
                } else if p.kind() == "inferred_parameters" {
                    let mut c = p.walk();
                    for id in p.named_children(&mut c) {
                        add(n, n, Some(id), None);
                    }
                }
            }
        }
        _ => {}
    }
    let mut cur = n.walk();
    for c in n.children(&mut cur) {
        collect_locals(c, src, out);
    }
}

fn lookup_field(name: &str, fields: &[Env]) -> Option<Receiver> {
    fields
        .iter()
        .rev()
        .find_map(|e| e.get(name))
        .and_then(Clone::clone)
}

/// Receiver denoted by an expression, when it is one of the RPC shapes.
/// `locals` are (declaration offset, name, receiver) already known before `at`.
fn receiver(n: Node, src: &str, locals: &[Decl], fields: &[Env]) -> Option<Receiver> {
    match n.kind() {
        "parenthesized_expression" => n
            .named_child(0)
            .and_then(|c| receiver(c, src, locals, fields)),
        // `(XAsync) GWT.create(X.class)`: the cast says it is a service twin.
        // A cast to any other type (ClientBundle, Constants...) is not RPC.
        "cast_expression" => {
            let twin = n
                .child_by_field_name("type")
                .and_then(|t| from_type(t, src))?;
            match n
                .child_by_field_name("value")
                .and_then(|v| receiver(v, src, locals, fields))
            {
                Some(Receiver::Sync { name, .. }) => Some(Receiver::Sync {
                    name,
                    strict: false,
                }),
                _ => Some(twin),
            }
        }
        "identifier" => {
            let name = text(n, src);
            let at = n.start_byte();
            let local = locals
                .iter()
                .filter(|(pos, (lo, hi), nm, _)| *pos <= at && *lo <= at && at < *hi && nm == name)
                .max_by_key(|(pos, ..)| *pos);
            match local {
                Some((.., r)) => r.clone(),
                None => lookup_field(name, fields),
            }
        }
        "field_access" => {
            let obj = n.child_by_field_name("object")?;
            let f = n.child_by_field_name("field")?;
            (obj.kind() == "this")
                .then(|| lookup_field(text(f, src), fields))
                .flatten()
        }
        "method_invocation" => {
            let obj = n.child_by_field_name("object")?;
            let name = text(n.child_by_field_name("name")?, src);
            if name == "create" && text(obj, src) == "GWT" {
                let args = n.child_by_field_name("arguments")?;
                let lit = args
                    .named_child(0)
                    .filter(|a| a.kind() == "class_literal")?;
                let ty = lit.named_child(0).and_then(|t| type_name(t, src))?;
                return Some(Receiver::Sync {
                    name: ty,
                    strict: true,
                });
            }
            // `X.Util.getInstance()`: a static call on a nested type of `X`.
            if obj.kind() == "field_access" {
                let x = obj.child_by_field_name("object")?;
                let nested = text(obj.child_by_field_name("field")?, src);
                let upper = |s: &str| s.chars().next().is_some_and(char::is_uppercase);
                if x.kind() == "identifier" && upper(text(x, src)) && upper(nested) {
                    return Some(Receiver::Sync {
                        name: text(x, src).to_string(),
                        strict: nested != "Util" || name != "getInstance",
                    });
                }
            }
            None
        }
        _ => None,
    }
}

fn visit(
    n: Node,
    src: &str,
    caller: &str,
    locals: &[Decl],
    fields: &[Env],
    out: &mut Vec<RawRpcCall>,
) {
    if n.kind() == "method_invocation"
        && let (Some(obj), Some(name)) = (
            n.child_by_field_name("object"),
            n.child_by_field_name("name"),
        )
    {
        let method = text(name, src);
        if !OBJECT_METHODS.contains(&method)
            && let Some(recv) = receiver(obj, src, locals, fields)
        {
            // Comments between arguments are named nodes too.
            let arity = n.child_by_field_name("arguments").map_or(0, |a| {
                let mut c = a.walk();
                a.named_children(&mut c)
                    .filter(|x| !matches!(x.kind(), "line_comment" | "block_comment"))
                    .count()
            });
            out.push(RawRpcCall {
                caller: caller.to_string(),
                recv,
                method: method.to_string(),
                arity: arity as u32,
            });
        }
    }
    let mut cur = n.walk();
    for c in n.children(&mut cur) {
        visit(c, src, caller, locals, fields, out);
    }
}

/// RPC-shaped calls in a method or constructor; `fields` are the field
/// environments of the enclosing types, outermost first.
pub fn collect(method: Node, src: &str, caller: &str, fields: &[Env], out: &mut Vec<RawRpcCall>) {
    let mut locals = Locals { decls: vec![] };
    collect_locals(method, src, &mut locals);
    visit(method, src, caller, &locals.decls, fields, out);
}
