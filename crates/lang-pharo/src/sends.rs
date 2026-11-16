//! Message-send extraction from a Smalltalk method body: a small lexer plus a
//! recursive-descent scanner. No type inference: only the receiver *kind* of
//! each send is kept (see [`Recv`]).

use structure_ir::ir::Range;

use super::{Lines, skip_balanced, skip_literal};

/// What a send's receiver is, syntactically.
#[derive(Debug, Clone, PartialEq)]
pub enum Recv {
    SelfRecv,
    Super,
    /// A capitalised identifier (may still be a class/pool variable).
    Class(String),
    /// Anything else, including the result of another send.
    Other,
}

/// One message send found in a method body.
#[derive(Debug, Clone, PartialEq)]
pub struct Send {
    /// Id of the calling method symbol.
    pub from: String,
    pub selector: String,
    pub recv: Recv,
    /// First selector token (first keyword part for keyword sends).
    pub range: Range,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum T {
    Ident,
    Keyword,
    Binary,
    Assign,
    Caret,
    Period,
    Semi,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Pipe,
    Colon,
    Literal,
}

#[derive(Clone, Copy, Debug)]
struct Tok {
    t: T,
    s: usize,
    e: usize,
}

fn is_binary_char(c: u8) -> bool {
    matches!(
        c,
        b'+' | b'-'
            | b'*'
            | b'/'
            | b'\\'
            | b'<'
            | b'>'
            | b'='
            | b'~'
            | b'@'
            | b'%'
            | b'&'
            | b'?'
            | b','
            | b'!'
    )
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn lex(b: &[u8]) -> Vec<Tok> {
    let n = b.len();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = b[i];
        let push = |out: &mut Vec<Tok>, t, s, e| out.push(Tok { t, s, e });
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'"' {
            i = skip_literal(b, i).unwrap_or(n);
        } else if c == b'\'' || c == b'$' {
            let e = skip_literal(b, i).unwrap_or(n);
            push(&mut out, T::Literal, i, e);
            i = e;
        } else if c == b'#' {
            let e = match b.get(i + 1) {
                Some(b'(') => skip_balanced(b, i + 1, b'(', b')').unwrap_or(n),
                Some(b'[') => skip_balanced(b, i + 1, b'[', b']').unwrap_or(n),
                Some(b'{') => skip_balanced(b, i + 1, b'{', b'}').unwrap_or(n),
                Some(b'\'') => skip_literal(b, i + 1).unwrap_or(n),
                Some(&x) if is_ident_char(x) => {
                    let mut e = i + 1;
                    while e < n
                        && (is_ident_char(b[e])
                            || b[e] == b':'
                            || (b[e] == b'.' && b.get(e + 1).is_some_and(|&x| is_ident_char(x))))
                    {
                        e += 1;
                    }
                    e
                }
                Some(&x) if is_binary_char(x) => {
                    let mut e = i + 1;
                    while e < n && is_binary_char(b[e]) {
                        e += 1;
                    }
                    e
                }
                _ => i + 1,
            };
            push(&mut out, T::Literal, i, e);
            i = e;
        } else if c.is_ascii_digit()
            || (c == b'-'
                && b.get(i + 1).is_some_and(u8::is_ascii_digit)
                && !out.last().is_some_and(|t| {
                    matches!(
                        t.t,
                        T::Ident | T::Literal | T::RParen | T::RBracket | T::RBrace
                    )
                }))
        {
            let mut e = i + 1;
            while e < n {
                if is_ident_char(b[e])
                    || (b[e] == b'.' && b.get(e + 1).is_some_and(u8::is_ascii_digit))
                {
                    e += 1;
                } else {
                    break;
                }
            }
            push(&mut out, T::Literal, i, e);
            i = e;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let mut e = i;
            while e < n && is_ident_char(b[e]) {
                e += 1;
            }
            if b.get(e) == Some(&b':') && b.get(e + 1) != Some(&b'=') {
                push(&mut out, T::Keyword, i, e + 1);
                i = e + 1;
            } else {
                push(&mut out, T::Ident, i, e);
                i = e;
            }
        } else if c == b':' {
            if b.get(i + 1) == Some(&b'=') {
                push(&mut out, T::Assign, i, i + 2);
                i += 2;
            } else {
                push(&mut out, T::Colon, i, i + 1);
                i += 1;
            }
        } else if is_binary_char(c) {
            let mut e = i + 1;
            while e < n && is_binary_char(b[e]) {
                e += 1;
            }
            push(&mut out, T::Binary, i, e);
            i = e;
        } else {
            let t = match c {
                b'^' => Some(T::Caret),
                b'.' => Some(T::Period),
                b';' => Some(T::Semi),
                b'(' => Some(T::LParen),
                b')' => Some(T::RParen),
                b'[' => Some(T::LBracket),
                b']' => Some(T::RBracket),
                b'{' => Some(T::LBrace),
                b'}' => Some(T::RBrace),
                b'|' => Some(T::Pipe),
                _ => None,
            };
            if let Some(t) = t {
                push(&mut out, t, i, i + 1);
            }
            i += 1;
        }
    }
    out
}

const MAX_DEPTH: usize = 200;

struct Scanner<'a> {
    toks: Vec<Tok>,
    b: &'a [u8],
    pos: usize,
    depth: usize,
    base: usize,
    from: &'a str,
    lines: &'a Lines,
    out: Vec<Send>,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<T> {
        self.toks.get(self.pos).map(|t| t.t)
    }

    fn peek_at(&self, k: usize) -> Option<T> {
        self.toks.get(self.pos + k).map(|t| t.t)
    }

    fn text(&self, t: Tok) -> &str {
        std::str::from_utf8(&self.b[t.s..t.e]).unwrap_or("")
    }

    /// Consumes the current token (caller checked it exists).
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).copied();
        self.pos += 1;
        t
    }

    fn is_binary(&self) -> bool {
        matches!(self.peek(), Some(T::Binary | T::Pipe))
    }

    fn send(&mut self, selector: String, recv: &Recv, tok: Tok) {
        self.out.push(Send {
            from: self.from.to_string(),
            selector,
            recv: recv.clone(),
            range: self.lines.range(self.base + tok.s, self.base + tok.e),
        });
    }

    fn statements(&mut self, close: T) {
        while let Some(t) = self.peek() {
            if t == close {
                break;
            }
            let start = self.pos;
            match t {
                T::Period | T::Caret => self.pos += 1,
                T::Pipe => {
                    // Temporaries declared mid-block.
                    self.pos += 1;
                    while self.peek() == Some(T::Ident) {
                        self.pos += 1;
                    }
                    if self.peek() == Some(T::Pipe) {
                        self.pos += 1;
                    }
                }
                T::Binary if self.toks.get(self.pos).map(|k| self.text(*k)) == Some("<") => {
                    // Pragma: `<primitive: 1>`, `<script>`.
                    while let Some(k) = self.bump() {
                        if k.t == T::Binary && self.text(k) == ">" {
                            break;
                        }
                    }
                }
                _ => {
                    self.expression();
                }
            }
            if self.pos == start {
                self.pos += 1;
            }
        }
    }

    fn block(&mut self) {
        self.pos += 1; // [
        if self.depth >= MAX_DEPTH {
            self.pos = self.toks.len();
            return;
        }
        self.depth += 1;
        let mut params = false;
        while self.peek() == Some(T::Colon) {
            params = true;
            self.pos += 1;
            if self.peek() == Some(T::Ident) {
                self.pos += 1;
            }
        }
        if params && self.peek() == Some(T::Pipe) {
            self.pos += 1;
        }
        self.statements(T::RBracket);
        if self.peek() == Some(T::RBracket) {
            self.pos += 1;
        }
        self.depth -= 1;
    }

    fn primary(&mut self) -> Recv {
        match self.peek() {
            Some(T::Ident) => {
                let Some(tok) = self.bump() else {
                    return Recv::Other;
                };
                let name = self.text(tok);
                match name {
                    "self" => Recv::SelfRecv,
                    "super" => Recv::Super,
                    _ if name.starts_with(|c: char| c.is_ascii_uppercase()) => {
                        Recv::Class(name.to_string())
                    }
                    _ => Recv::Other,
                }
            }
            Some(T::Literal) => {
                self.pos += 1;
                Recv::Other
            }
            Some(T::LParen) => {
                self.pos += 1;
                if self.depth >= MAX_DEPTH {
                    self.pos = self.toks.len();
                    return Recv::Other;
                }
                self.depth += 1;
                self.expression();
                if self.peek() == Some(T::RParen) {
                    self.pos += 1;
                }
                self.depth -= 1;
                Recv::Other
            }
            Some(T::LBracket) => {
                self.block();
                Recv::Other
            }
            Some(T::LBrace) => {
                self.pos += 1;
                if self.depth >= MAX_DEPTH {
                    self.pos = self.toks.len();
                    return Recv::Other;
                }
                self.depth += 1;
                self.statements(T::RBrace);
                if self.peek() == Some(T::RBrace) {
                    self.pos += 1;
                }
                self.depth -= 1;
                Recv::Other
            }
            _ => Recv::Other,
        }
    }

    /// Unary sends on `recv`; `last` tracks the receiver of the last message.
    fn unary_tail(&mut self, mut recv: Recv, last: &mut Recv) -> Recv {
        while self.peek() == Some(T::Ident) && self.peek_at(1) != Some(T::Assign) {
            let Some(tok) = self.bump() else { break };
            let sel = self.text(tok).to_string();
            self.send(sel, &recv, tok);
            *last = std::mem::replace(&mut recv, Recv::Other);
        }
        recv
    }

    fn operand(&mut self) -> Recv {
        let p = self.primary();
        self.unary_tail(p, &mut Recv::Other)
    }

    fn binary_tail(&mut self, mut recv: Recv, last: &mut Recv) -> Recv {
        while self.is_binary() {
            let Some(tok) = self.bump() else { break };
            self.operand();
            let sel = self.text(tok).to_string();
            self.send(sel, &recv, tok);
            *last = std::mem::replace(&mut recv, Recv::Other);
        }
        recv
    }

    /// A keyword argument: primary, unary then binary sends.
    fn binary_expr(&mut self) {
        let p = self.primary();
        let mut last = Recv::Other;
        let r = self.unary_tail(p, &mut last);
        self.binary_tail(r, &mut last);
    }

    /// Keyword parts and arguments; returns the selector and first part.
    fn keyword_message(&mut self) -> Option<(String, Tok)> {
        let first = *self.toks.get(self.pos)?;
        let mut sel = String::new();
        while self.peek() == Some(T::Keyword) {
            let Some(tok) = self.bump() else { break };
            sel.push_str(self.text(tok));
            self.binary_expr();
        }
        Some((sel, first))
    }

    fn expression(&mut self) -> Recv {
        while self.peek() == Some(T::Ident) && self.peek_at(1) == Some(T::Assign) {
            self.pos += 2;
        }
        let p = self.primary();
        let mut last = p.clone();
        let r = self.unary_tail(p, &mut last);
        let mut r = self.binary_tail(r, &mut last);
        if self.peek() == Some(T::Keyword) {
            if let Some((sel, first)) = self.keyword_message() {
                self.send(sel, &r, first);
            }
            last = std::mem::replace(&mut r, Recv::Other);
        }
        while self.peek() == Some(T::Semi) {
            self.pos += 1;
            self.cascade_message(&last);
        }
        r
    }

    /// One cascaded message, sent to the receiver of the cascade.
    fn cascade_message(&mut self, recv: &Recv) {
        match self.peek() {
            Some(T::Ident) => {
                if let Some(tok) = self.bump() {
                    let sel = self.text(tok).to_string();
                    self.send(sel, recv, tok);
                }
            }
            Some(T::Binary | T::Pipe) => {
                if let Some(tok) = self.bump() {
                    self.operand();
                    let sel = self.text(tok).to_string();
                    self.send(sel, recv, tok);
                }
            }
            Some(T::Keyword) => {
                if let Some((sel, first)) = self.keyword_message() {
                    self.send(sel, recv, first);
                }
            }
            _ => {}
        }
    }
}

/// Sends of a method body (`body` starts at its `[`, `base` is its absolute
/// byte offset in the file), in source order.
pub(crate) fn extract(from: &str, body: &str, base: usize, lines: &Lines) -> Vec<Send> {
    let b = body.as_bytes();
    let mut sc = Scanner {
        toks: lex(b),
        b,
        pos: 0,
        depth: 0,
        base,
        from,
        lines,
        out: Vec::new(),
    };
    if sc.peek() == Some(T::LBracket) {
        sc.block();
    }
    let mut out = sc.out;
    out.sort_by_key(|s| (s.range.start_line, s.range.start_col));
    out
}
