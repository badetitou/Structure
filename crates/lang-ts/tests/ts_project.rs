use std::fs;
use std::path::{Path, PathBuf};

use structure_ir::ir::{EdgeKind, Graph};
use structure_lang_ts::parse_project;

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("structure-ts-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn has(g: &Graph, from: &str, to: &str, kind: EdgeKind) -> bool {
    g.edges
        .iter()
        .any(|e| e.from == from && e.to == to && e.kind == kind)
}

#[test]
fn walk_filters() {
    let r = tmp("walk");
    write(&r, "a.ts", "export class A {}");
    write(&r, "node_modules/x/n.ts", "export class N {}");
    write(&r, "dist/d.ts", "export class D {}");
    write(&r, ".angular/c.ts", "export class C {}");
    write(&r, "t.d.ts", "export class Dts {}");
    write(&r, "s.spec.ts", "export class S {}");
    write(&r, "u.test.tsx", "export class T {}");
    write(&r, "ui/B.tsx", "export const B = () => <div/>;");
    let g = parse_project(&r).unwrap();
    let ids: Vec<&str> = g.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["a.ts#A", "ui/B.tsx#B"]);
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn heritage_and_uses_resolution() {
    let r = tmp("res");
    write(
        &r,
        "src/base.ts",
        "export class Base {}\nexport interface Api {}\ntype Alias = string;\nexport class Api2 {}\n",
    );
    write(&r, "src/lib/index.ts", "export class Lib {}\n");
    write(&r, "src/a/x.ts", "export class Svc {}\n");
    write(
        &r,
        "src/a/main.ts",
        "import { Base, Api as MyApi, Alias } from '../base.js';\nimport { Lib } from '../lib';\nimport { Svc } from './x';\nimport { HttpClient } from '@angular/common/http';\n\
class Local {}\n\
export class Main<T> extends Base implements MyApi, Local {\n  constructor(private s: Svc, private l: Lib, h: HttpClient, t: T, a: Alias, e: HTMLElement) {}\n}\n",
    );
    write(
        &r,
        "root.ts",
        "import { Base } from './src/base';\nexport class R extends Base {}\n",
    );
    let g = parse_project(&r).unwrap();
    let m = "src/a/main.ts#Main";
    assert!(has(&g, m, "src/base.ts#Base", EdgeKind::Extends));
    assert!(has(&g, m, "src/base.ts#Api", EdgeKind::Implements));
    assert!(has(&g, "root.ts#R", "src/base.ts#Base", EdgeKind::Extends));
    assert!(has(&g, m, "src/a/x.ts#Svc", EdgeKind::Uses));
    assert!(has(&g, m, "src/lib/index.ts#Lib", EdgeKind::Uses));
    assert!(has(&g, m, "ext:HttpClient", EdgeKind::Uses));
    assert!(!g.edges.iter().any(|e| e.to == "ext:T"
        || e.to == "ext:Alias"
        || e.to == "ext:HTMLElement"
        || e.to == "ext:Local"));
    // Deterministic order.
    let ids: Vec<&String> = g.symbols.iter().map(|s| &s.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn same_file_heritage() {
    let r = tmp("same");
    write(
        &r,
        "a.ts",
        "interface I {}\nclass B {}\nexport class C extends B implements I {}\n",
    );
    let g = parse_project(&r).unwrap();
    assert!(has(&g, "a.ts#C", "a.ts#B", EdgeKind::Extends));
    assert!(has(&g, "a.ts#C", "a.ts#I", EdgeKind::Implements));
    let _ = fs::remove_dir_all(&r);
}
