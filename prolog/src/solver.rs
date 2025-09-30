use crate::{
    types::*,
    unify::{apply, unify},
};
use std::collections::HashMap;

fn standardize_apart(cl: &Clause, counter: &mut u64) -> Clause {
    *counter += 1;
    let id = *counter;
    let mut ren: HashMap<String, String> = HashMap::new();

    fn rename(t: &Term, ren: &mut HashMap<String, String>, id: u64) -> Term {
        match t {
            Term::Var(v) => {
                let nv = ren
                    .entry(v.clone())
                    .or_insert_with(|| format!("{}#{}", v, id))
                    .clone();
                Term::Var(nv)
            }
            Term::Struct { functor, args } => Term::Struct {
                functor: functor.clone(),
                args: args.iter().map(|x| rename(x, ren, id)).collect(),
            },
            Term::List { items, tail } => Term::List {
                items: items.iter().map(|x| rename(x, ren, id)).collect(),
                tail: tail.as_ref().map(|b| Box::new(rename(b, ren, id))),
            },
            Term::Atom(a) => Term::Atom(a.clone()),
            Term::Number(n) => Term::Number(*n),
        }
    }

    match cl {
        Clause::Fact(h) => Clause::Fact(rename(h, &mut ren, id)),
        Clause::Rule { head, body } => Clause::Rule {
            head: rename(head, &mut ren, id),
            body: body.iter().map(|t| rename(t, &mut ren, id)).collect(),
        },
    }
}

pub fn solve(kb: &[Clause], query: Vec<Term>) -> Vec<Subst> {
    let mut out = Vec::new();
    let mut cid = 0;
    dfs(kb, query, &Subst::new(), &mut out, &mut cid);
    out
}

fn dfs(kb: &[Clause], goals: Vec<Term>, s: &Subst, out: &mut Vec<Subst>, cid: &mut u64) {
    if goals.is_empty() {
        out.push(s.clone());
        return;
    }
    let g0 = apply(&goals[0], s); // 代入を反映した現在の先頭ゴール
    for cl in kb {
        let cl = standardize_apart(cl, cid);
        let (head, body) = match cl {
            Clause::Fact(h) => (h, vec![]),
            Clause::Rule { head, body } => (head, body),
        };
        if let Some(s1) = unify(&g0, &head, s) {
            // 新しいゴール列：規則本体 + 残りゴール。すぐに s1 を適用して正規化
            let mut new_goals: Vec<Term> = body.into_iter().collect();
            new_goals.extend(goals.iter().skip(1).map(|t| apply(t, &s1)));
            dfs(kb, new_goals, &s1, out, cid);
        }
    }
}

#[cfg(test)]
mod run_query {
    use super::*;

    fn v(x: &str) -> Term {
        Term::Var(x.to_string())
    }
    fn a(x: &str) -> Term {
        Term::Atom(x.to_string())
    }
    fn f(functor: &str, args: Vec<Term>) -> Term {
        Term::Struct {
            functor: functor.to_string(),
            args,
        }
    }

    #[test]
    fn ancestor_demo() {
        let kb = vec![
            Clause::Fact(f("parent", vec![a("a"), a("b")])),
            Clause::Fact(f("parent", vec![a("b"), a("c")])),
            Clause::Rule {
                head: f("ancestor", vec![v("X"), v("Y")]),
                body: vec![f("parent", vec![v("X"), v("Y")])],
            },
            Clause::Rule {
                head: f("ancestor", vec![v("X"), v("Y")]),
                body: vec![
                    f("parent", vec![v("X"), v("Z")]),
                    f("ancestor", vec![v("Z"), v("Y")]),
                ],
            },
        ];

        let sols = solve(&kb, vec![f("ancestor", vec![a("a"), v("Y")])]);
        assert!(sols.len() >= 2);
        assert_eq!(apply(&v("Y"), &sols[0]), a("b"));
        assert_eq!(apply(&v("Y"), &sols[1]), a("c"));
    }
}
