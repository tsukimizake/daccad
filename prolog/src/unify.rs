use crate::types::Term;
use std::collections::HashMap;

type Subst = HashMap<String, Term>;

fn apply(term: &Term, s: &Subst) -> Term {
    match term {
        Term::Var(v) => {
            if let Some(t) = s.get(v) {
                apply(t, s) // 連鎖代入を正規化
            } else {
                term.clone()
            }
        }
        Term::Struct { functor, args } => Term::Struct {
            functor: functor.clone(),
            args: args.iter().map(|t| apply(t, s)).collect(),
        },
        Term::List { items, tail } => Term::List {
            items: items.iter().map(|t| apply(t, s)).collect(),
            tail: tail.as_ref().map(|b| Box::new(apply(b, s))),
        },
        Term::Atom(_) | Term::Number(_) => term.clone(),
    }
}

fn occurs_in(var: &str, term: &Term, s: &Subst) -> bool {
    let t = apply(term, s);
    match t {
        Term::Var(ref v) => v == var,
        Term::Struct { args, .. } => args.iter().any(|t| occurs_in(var, t, s)),
        Term::List {
            ref items,
            ref tail,
        } => {
            items.iter().any(|t| occurs_in(var, t, s))
                || tail.as_ref().map_or(false, |t| occurs_in(var, t, s))
        }
        Term::Atom(_) | Term::Number(_) => false,
    }
}

fn bind(var: &str, term: &Term, mut s: Subst) -> Option<Subst> {
    if let Term::Var(v2) = term {
        if v2 == var {
            return Some(s); // X = X
        }
    }
    if occurs_in(var, term, &s) {
        return None; // 発生検査
    }
    s.insert(var.to_string(), apply(term, &s));
    Some(s)
}

pub fn unify(t1: &Term, t2: &Term, s0: &Subst) -> Option<Subst> {
    let t1 = apply(t1, s0);
    let t2 = apply(t2, s0);

    match (t1, t2) {
        (Term::Var(x), t) => bind(&x, &t, s0.clone()),
        (t, Term::Var(x)) => bind(&x, &t, s0.clone()),

        (Term::Atom(a), Term::Atom(b)) if a == b => Some(s0.clone()),
        (Term::Number(a), Term::Number(b)) if a == b => Some(s0.clone()),

        (
            Term::Struct {
                functor: f1,
                args: a1,
            },
            Term::Struct {
                functor: f2,
                args: a2,
            },
        ) if f1 == f2 && a1.len() == a2.len() => a1
            .into_iter()
            .zip(a2.into_iter())
            .try_fold(s0.clone(), |s, (l, r)| unify(&l, &r, &s)),

        (
            Term::List {
                items: i1,
                tail: t1,
            },
            Term::List {
                items: i2,
                tail: t2,
            },
        ) => {
            // 先頭から要素を突き合わせ、どちらかが尽きたらtailで処理
            let mut s = s0.clone();
            let mut left = i1;
            let mut right = i2;

            let n = left.len().min(right.len());
            for k in 0..n {
                s = unify(&left[k], &right[k], &s)?;
            }
            // 長さが違う場合は不足分をtailに押し込む
            match (left.split_off(n), right.split_off(n), t1, t2) {
                (rem_l, rem_r, tl, tr) => {
                    let rest_l = if rem_l.is_empty() {
                        tl.map(|b| *b).unwrap_or(Term::List {
                            items: vec![],
                            tail: None,
                        })
                    } else {
                        Term::List {
                            items: rem_l,
                            tail: tl,
                        }
                    };
                    let rest_r = if rem_r.is_empty() {
                        tr.map(|b| *b).unwrap_or(Term::List {
                            items: vec![],
                            tail: None,
                        })
                    } else {
                        Term::List {
                            items: rem_r,
                            tail: tr,
                        }
                    };
                    unify(&rest_l, &rest_r, &s)
                }
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(name: &str) -> Term {
        Term::Var(name.to_string())
    }
    fn a(name: &str) -> Term {
        Term::Atom(name.to_string())
    }

    #[test]
    fn unify_var_atom() {
        let s0 = Subst::new();
        let s = unify(&v("X"), &a("a"), &s0).unwrap();
        assert_eq!(apply(&v("X"), &s), a("a"));
    }

    #[test]
    fn occurs_check_blocks_cycle() {
        let s0 = Subst::new();
        let t = Term::Struct {
            functor: "f".into(),
            args: vec![v("X")],
        };
        assert!(unify(&v("X"), &t, &s0).is_none());
    }

    #[test]
    fn unify_lists_with_tail() {
        let s0 = Subst::new();
        let left = Term::List {
            items: vec![v("X"), Term::Number(2)],
            tail: Some(Box::new(v("T"))),
        };
        let right = Term::List {
            items: vec![Term::Number(1), Term::Number(2), Term::Number(3)],
            tail: None,
        };
        let s = unify(&left, &right, &s0).unwrap();
        assert_eq!(apply(&v("X"), &s), Term::Number(1));
        assert_eq!(
            apply(&v("T"), &s),
            Term::List {
                items: vec![Term::Number(3)],
                tail: None
            }
        );
    }
}
