//! HTTP requests made from TypeScript code: Angular `HttpClient`, `fetch`, `axios`.

use std::collections::HashSet;

use tree_sitter::Node;

use structure_ir::ir::{HttpEndpoint, HttpVerb};

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    n.utf8_text(src.as_bytes()).unwrap_or("")
}

fn is_inject_http_client(v: Node, src: &str) -> bool {
    if v.kind() != "call_expression" {
        return false;
    }
    let (Some(f), Some(args)) = (
        v.child_by_field_name("function"),
        v.child_by_field_name("arguments"),
    ) else {
        return false;
    };
    let mut c = args.walk();
    text(f, src) == "inject"
        && args
            .named_children(&mut c)
            .next()
            .is_some_and(|a| text(a, src) == "HttpClient")
}

/// True when the annotation's own type is `HttpClient` (also generic or in a union).
fn mentions_http_client(n: Node, src: &str) -> bool {
    match n.kind() {
        "type_identifier" => text(n, src) == "HttpClient",
        "generic_type" => n
            .child_by_field_name("name")
            .is_some_and(|t| mentions_http_client(t, src)),
        "type_annotation" | "parenthesized_type" | "union_type" => {
            let mut c = n.walk();
            n.named_children(&mut c)
                .any(|ch| mentions_http_client(ch, src))
        }
        _ => false,
    }
}

/// Names of parameters, fields and variables under `n` that are typed
/// `HttpClient` or initialised with `inject(HttpClient)`.
pub fn http_client_names(n: Node, src: &str, out: &mut HashSet<String>) {
    if matches!(
        n.kind(),
        "required_parameter"
            | "optional_parameter"
            | "public_field_definition"
            | "variable_declarator"
    ) {
        let name = n
            .child_by_field_name("name")
            .or_else(|| n.child_by_field_name("pattern"));
        let typed = n
            .child_by_field_name("type")
            .is_some_and(|t| mentions_http_client(t, src));
        let injected = n
            .child_by_field_name("value")
            .is_some_and(|v| is_inject_http_client(v, src));
        if let Some(name) = name
            && (typed || injected)
        {
            out.insert(text(name, src).to_string());
        }
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        http_client_names(ch, src, out);
    }
}

/// HTTP calls made under `n` (nested callbacks included).
pub fn http_calls(n: Node, src: &str, clients: &HashSet<String>, out: &mut Vec<HttpEndpoint>) {
    if n.kind() == "call_expression"
        && let Some(call) = call_at(n, src, clients)
    {
        out.push(call);
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        http_calls(ch, src, clients, out);
    }
}

fn verb_of(s: &str) -> HttpVerb {
    match s.to_ascii_lowercase().as_str() {
        "get" => HttpVerb::Get,
        "post" => HttpVerb::Post,
        "put" => HttpVerb::Put,
        "delete" => HttpVerb::Delete,
        "patch" => HttpVerb::Patch,
        _ => HttpVerb::Any,
    }
}

fn is_client(recv: Node, src: &str, clients: &HashSet<String>) -> bool {
    let name = match recv.kind() {
        "identifier" => text(recv, src),
        "member_expression" => match recv.child_by_field_name("property") {
            Some(p) => text(p, src),
            None => return false,
        },
        _ => return false,
    };
    name == "http" || clients.contains(name)
}

fn call_at(n: Node, src: &str, clients: &HashSet<String>) -> Option<HttpEndpoint> {
    let f = n.child_by_field_name("function")?;
    let args = n.child_by_field_name("arguments")?;
    let mut c = args.walk();
    let a: Vec<Node> = args.named_children(&mut c).collect();
    let url = |i: usize| a.get(i).map_or("?".to_string(), |x| url_of(*x, src));
    match f.kind() {
        "identifier" => match text(f, src) {
            "fetch" => {
                let verb = object_method(a.get(1).copied(), src);
                Some(HttpEndpoint { verb, path: url(0) })
            }
            "axios" => {
                let first = *a.first()?;
                if first.kind() == "object" {
                    let path = prop(first, "url", src).map_or("?".to_string(), |u| url_of(u, src));
                    Some(HttpEndpoint {
                        verb: object_method(Some(first), src),
                        path,
                    })
                } else if a.len() > 1 {
                    Some(HttpEndpoint {
                        verb: object_method(a.get(1).copied(), src),
                        path: url(0),
                    })
                } else if raw_url(first, src).is_some() {
                    Some(HttpEndpoint {
                        verb: HttpVerb::Get,
                        path: url(0),
                    })
                } else {
                    // `axios(config)`: method unknown.
                    Some(HttpEndpoint {
                        verb: HttpVerb::Any,
                        path: "?".to_string(),
                    })
                }
            }
            _ => None,
        },
        "member_expression" => {
            let recv = f.child_by_field_name("object")?;
            let name = text(f.child_by_field_name("property")?, src);
            let is_verb = matches!(name, "get" | "post" | "put" | "delete" | "patch");
            if is_verb && (text(recv, src) == "axios" || is_client(recv, src, clients)) {
                return Some(HttpEndpoint {
                    verb: verb_of(name),
                    path: url(0),
                });
            }
            if name == "request" && is_client(recv, src, clients) {
                let verb = a
                    .first()
                    .and_then(|v| literal(*v, src))
                    .map_or(HttpVerb::Any, |v| verb_of(&v));
                return Some(HttpEndpoint { verb, path: url(1) });
            }
            None
        }
        _ => None,
    }
}

/// Value node of property `key` in an object literal.
fn prop<'t>(obj: Node<'t>, key: &str, src: &str) -> Option<Node<'t>> {
    if obj.kind() != "object" {
        return None;
    }
    let mut c = obj.walk();
    obj.named_children(&mut c).find_map(|p| {
        if p.kind() != "pair" {
            return None;
        }
        let k = p.child_by_field_name("key")?;
        (text(k, src).trim_matches(['"', '\'']) == key)
            .then(|| p.child_by_field_name("value"))
            .flatten()
    })
}

/// Verb of an options argument. Get only when it is absent or an object literal
/// with no `method` key and no spread; Any when the method cannot be read.
fn object_method(obj: Option<Node>, src: &str) -> HttpVerb {
    let Some(obj) = obj else {
        return HttpVerb::Get;
    };
    if obj.kind() != "object" {
        return HttpVerb::Any;
    }
    let mut c = obj.walk();
    for ch in obj.named_children(&mut c) {
        match ch.kind() {
            "spread_element" => return HttpVerb::Any,
            "shorthand_property_identifier" if text(ch, src) == "method" => {
                return HttpVerb::Any;
            }
            _ => {}
        }
    }
    match prop(obj, "method", src) {
        None => HttpVerb::Get,
        Some(m) => literal(m, src).map_or(HttpVerb::Any, |s| verb_of(&s)),
    }
}

/// Plain string literal content.
fn literal(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "string" => Some(text(n, src).trim_matches(['"', '\'']).to_string()),
        "template_string" if !text(n, src).contains("${") => {
            Some(text(n, src).trim_matches('`').to_string())
        }
        _ => None,
    }
}

fn placeholder(n: Node, src: &str) -> String {
    match n.kind() {
        "identifier" => format!("{{{}}}", text(n, src)),
        "member_expression" => match n.child_by_field_name("property") {
            Some(p) => format!("{{{}}}", text(p, src)),
            None => "{}".into(),
        },
        "parenthesized_expression" | "non_null_expression" | "as_expression" => {
            let mut c = n.walk();
            match n.named_children(&mut c).next() {
                Some(i) => placeholder(i, src),
                None => "{}".into(),
            }
        }
        _ => "{}".into(),
    }
}

/// Raw URL text with placeholders; None when it is not a literal-based form.
fn raw_url(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "string" => Some(text(n, src).trim_matches(['"', '\'']).to_string()),
        "template_string" => {
            let mut s = String::new();
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                match ch.kind() {
                    "`" => {}
                    "template_substitution" => {
                        let mut c2 = ch.walk();
                        match ch.named_children(&mut c2).next() {
                            Some(e) => s.push_str(&placeholder(e, src)),
                            None => s.push_str("{}"),
                        }
                    }
                    _ => s.push_str(text(ch, src)),
                }
            }
            Some(s)
        }
        "binary_expression"
            if n.child_by_field_name("operator").map(|o| text(o, src)) == Some("+") =>
        {
            let l = n.child_by_field_name("left")?;
            let r = n.child_by_field_name("right")?;
            Some(operand(l, src)? + &operand(r, src)?)
        }
        "parenthesized_expression" => {
            let mut c = n.walk();
            raw_url(n.named_children(&mut c).next()?, src)
        }
        _ => None,
    }
}

/// A concatenation operand: literal-based, identifier or member.
fn operand(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "identifier" | "member_expression" => Some(placeholder(n, src)),
        _ => raw_url(n, src),
    }
}

fn url_of(n: Node, src: &str) -> String {
    raw_url(n, src).map_or_else(|| "?".to_string(), |r| normalize(&r))
}

/// Leading `/` (unless a scheme is given), single `/`, no trailing `/`, no query.
pub fn normalize(raw: &str) -> String {
    let raw = raw.split(['?', '#']).next().unwrap_or("");
    let (scheme, rest) = match raw.find("://") {
        Some(i) if raw[..i].chars().all(|c| c.is_ascii_alphabetic()) => raw.split_at(i + 3),
        _ => ("", raw),
    };
    let body = rest
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if scheme.is_empty() {
        format!("/{body}")
    } else {
        format!("{scheme}{body}")
    }
}
