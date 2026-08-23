//! 同名重複の検査 (fail fast、型推論より前)。
//!
//! - top-level binding (`Decl::Value`) の同名重複。評価は依存順・cross-sketch
//!   参照は名前表で解決するため、同名が複数あるとどれを指すか定まらない。
//! - record リテラル / record update の field 重複。runtime の field 参照は
//!   先勝ち・sketch export は名前表 (後勝ち) なので、重複すると解釈が食い違う。

use crate::diagnostic::Diagnostic;
use crate::syntax::ast::*;
use std::collections::HashSet;

pub fn check_module(m: &Module) -> Vec<Diagnostic> {
    let mut diag = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for d in &m.decls {
        if let Decl::Value(v) = d
            && !seen.insert(v.name.as_str())
        {
            diag.push(Diagnostic::Duplicate {
                span: v.span,
                message: format!("top-level binding `{}` が重複しています", v.name),
            });
        }
    }
    for d in &m.decls {
        match d {
            Decl::Value(v) => check_records(&v.body, &mut diag),
            Decl::Slider(s) => check_records(&s.body, &mut diag),
            _ => {}
        }
    }
    diag
}

fn dup_fields(fields: &[RecordField], diag: &mut Vec<Diagnostic>) {
    let mut seen: HashSet<&str> = HashSet::new();
    for f in fields {
        if !seen.insert(f.name.as_str()) {
            diag.push(Diagnostic::Duplicate {
                span: f.span,
                message: format!("record の field `{}` が重複しています", f.name),
            });
        }
    }
}

fn check_records(e: &Expr, diag: &mut Vec<Diagnostic>) {
    match e {
        Expr::Var { .. } | Expr::Ctor { .. } | Expr::Lit(..) | Expr::Error(_) => {}
        Expr::Record(fields, _) => {
            dup_fields(fields, diag);
            fields.iter().for_each(|f| check_records(&f.value, diag));
        }
        Expr::RecordUpdate { base, updates, .. } => {
            dup_fields(updates, diag);
            check_records(base, diag);
            updates.iter().for_each(|f| check_records(&f.value, diag));
        }
        Expr::List(items, _) => items.iter().for_each(|x| check_records(x, diag)),
        Expr::Field { receiver, .. } => check_records(receiver, diag),
        Expr::App { func, arg, .. } => {
            check_records(func, diag);
            check_records(arg, diag);
        }
        Expr::Lambda { body, .. } => check_records(body, diag),
        Expr::Let { bindings, body, .. } => {
            bindings.iter().for_each(|b| check_records(&b.body, diag));
            check_records(body, diag);
        }
        Expr::Sketch { bindings, body, .. } => {
            bindings.iter().for_each(|b| check_records(&b.body, diag));
            check_records(body, diag);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            check_records(cond, diag);
            check_records(then_branch, diag);
            check_records(else_branch, diag);
        }
        Expr::Case {
            scrutinee, arms, ..
        } => {
            check_records(scrutinee, diag);
            for a in arms {
                if let Some(g) = &a.guard {
                    check_records(g, diag);
                }
                check_records(&a.body, diag);
            }
        }
        Expr::BinOp { left, right, .. } => {
            check_records(left, diag);
            check_records(right, diag);
        }
        Expr::Negate(inner, _) => check_records(inner, diag),
        Expr::Range { lo, hi, .. } => {
            check_records(lo, diag);
            check_records(hi, diag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse::parse;

    fn check_src(src: &str) -> Vec<Diagnostic> {
        let m = parse(src).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
        check_module(&m)
    }

    #[test]
    fn duplicate_top_level_binding_rejected() {
        let diags = check_src("a = 1.0\nb = 2.0\na = 3.0\n");
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(diags[0].message().contains("`a` が重複"), "{diags:?}");
    }

    #[test]
    fn duplicate_sketch_binding_rejected() {
        let diags = check_src(
            "sk = sketch\n    p = p2 1.0 1.0\n    in p\nend\nsk = sketch\n    q = p2 2.0 2.0\n    in q\nend\n",
        );
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(diags[0].message().contains("`sk` が重複"), "{diags:?}");
    }

    #[test]
    fn duplicate_record_field_rejected() {
        let diags = check_src("r = { a = 1.0, b = 2.0, a = 3.0 }\n");
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(diags[0].message().contains("field `a` が重複"), "{diags:?}");
    }

    #[test]
    fn duplicate_sketch_body_field_rejected() {
        let diags = check_src(
            "sk = sketch\n    pa = p2 1.0 0.0\n    pb = p2 9.0 0.0\n    in { a = pa, a = pb }\nend\n",
        );
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(diags[0].message().contains("field `a` が重複"), "{diags:?}");
    }

    #[test]
    fn duplicate_record_update_field_rejected() {
        let diags = check_src("f r = { r | a = 1.0, a = 2.0 }\n");
        assert_eq!(diags.len(), 1, "{diags:?}");
    }

    #[test]
    fn distinct_names_ok() {
        let diags =
            check_src("a = 1.0\nb = { x = 1.0, y = 2.0 }\nf x = x\ng = f { x = 1.0, y = 2.0 }\n");
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn signature_and_value_pair_ok() {
        let diags = check_src("a : Float\na = 1.0\n");
        assert!(diags.is_empty(), "{diags:?}");
    }
}
