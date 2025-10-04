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
    use crate::parse::{program, query};
    use crate::unify::apply;

    fn parse_program(src: &str) -> Vec<Clause> {
        let (_, clauses) = program(src).expect("Failed to parse program");
        clauses
    }

    fn parse_query(src: &str) -> Vec<Term> {
        let (_, goals) = query(src).expect("Failed to parse query");
        goals
    }

    fn test_query(program_src: &str, query_src: &str, expected: &[(&str, &str)]) {
        let kb = parse_program(program_src);
        let query_goals = parse_query(query_src);
        let sols = solve(&kb, query_goals);

        assert!(
            !sols.is_empty(),
            "No solutions found for query: {}",
            query_src
        );

        let found_match = sols.iter().any(|sol| {
            expected.iter().all(|(var_name, expected_value)| {
                let var_term = Term::Var(var_name.to_string());
                let actual_value = apply(&var_term, sol);
                let expected_term = Term::Atom(expected_value.to_string());
                actual_value == expected_term
            })
        });

        assert!(
            found_match,
            "Expected solution not found. Query: {}, Expected: {:?}, Found solutions: {:?}",
            query_src, expected, sols
        );
    }

    fn test_query_count(program_src: &str, query_src: &str, expected_count: usize) {
        let kb = parse_program(program_src);
        let query_goals = parse_query(query_src);
        let sols = solve(&kb, query_goals);

        assert_eq!(
            sols.len(),
            expected_count,
            "Expected {} solutions for query '{}', but found {}. Solutions: {:?}",
            expected_count,
            query_src,
            sols.len(),
            sols
        );
    }

    fn test_query_succeeds(program_src: &str, query_src: &str) {
        let kb = parse_program(program_src);
        let query_goals = parse_query(query_src);
        let sols = solve(&kb, query_goals);

        assert!(
            !sols.is_empty(),
            "Query should succeed but no solutions found: {}",
            query_src
        );
    }

    fn test_query_fails(program_src: &str, query_src: &str) {
        let kb = parse_program(program_src);
        let query_goals = parse_query(query_src);
        let sols = solve(&kb, query_goals);

        assert!(
            sols.is_empty(),
            "Query should fail but found solutions: {:?}",
            sols
        );
    }

    fn test_query_variable_values(
        program_src: &str,
        query_src: &str,
        var_name: &str,
        expected_values: &[&str],
    ) {
        let kb = parse_program(program_src);
        let query_goals = parse_query(query_src);
        let sols = solve(&kb, query_goals);

        assert!(
            !sols.is_empty(),
            "No solutions found for query: {}",
            query_src
        );

        let var_term = Term::Var(var_name.to_string());
        let actual_values: Vec<Term> = sols.iter().map(|sol| apply(&var_term, sol)).collect();

        for expected_value in expected_values {
            let expected_term = Term::Atom(expected_value.to_string());
            assert!(
                actual_values.contains(&expected_term),
                "Expected value '{}' for variable '{}' not found. Query: {}, Found values: {:?}",
                expected_value,
                var_name,
                query_src,
                actual_values
            );
        }
    }

    #[test]
    fn ancestor() {
        let program_str = r#"
            parent(a, b).
            parent(b, c).
            ancestor(X, Y) :- parent(X, Y).
            ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).
        "#;

        test_query(program_str, "ancestor(a, Y).", &[("Y", "b")]);
        test_query(program_str, "ancestor(a, Y).", &[("Y", "c")]);

        test_query_count(program_str, "ancestor(a, Y).", 2);
    }

    #[test]
    fn fact() {
        let program_str = r#"
            likes(mary, food).
            likes(mary, wine).
            likes(john, wine).
            likes(john, mary).
        "#;

        test_query(program_str, "likes(mary, food).", &[]);
        test_query(program_str, "likes(mary, X).", &[("X", "food")]);
        test_query(program_str, "likes(mary, X).", &[("X", "wine")]);
        test_query_count(program_str, "likes(mary, X).", 2);

        test_query_fails(program_str, "likes(mary, john).");
        test_query_fails(program_str, "likes(mary, beer).");
        test_query_fails(program_str, "likes(alice, X).");
    }

    #[test]
    fn peano() {
        let program_str = r#"
            add(zero, X, X).
            add(succ(X), Y, succ(Z)) :- add(X, Y, Z).
        "#;

        test_query_succeeds(program_str, "add(zero, succ(zero), succ(zero)).");
        test_query_succeeds(program_str, "add(succ(zero), succ(zero), Result).");

        test_query_count(program_str, "add(succ(zero), succ(zero), Result).", 1);
    }

    #[test]
    fn single_variable_single_value() {
        test_query(
            "parent(tom, bob). parent(bob, liz).",
            "parent(tom, X).",
            &[("X", "bob")],
        );
    }

    #[test]
    fn multiple_variables_one_solution() {
        test_query(
            "family(father(john), mother(mary), child(alice)).",
            "family(father(X), mother(Y), child(Z)).",
            &[("X", "john"), ("Y", "mary"), ("Z", "alice")],
        );
    }

    #[test]
    fn relationship_with_multiple_variables() {
        test_query(
            "married(john, mary).",
            "married(X, Y).",
            &[("X", "john"), ("Y", "mary")],
        );
    }

    #[test]
    fn person_with_multiple_attributes() {
        test_query(
            "person(alice, age(twentyfive), city(tokyo)).",
            "person(Name, age(Age), city(City)).",
            &[("Name", "alice"), ("Age", "twentyfive"), ("City", "tokyo")],
        );
    }

    #[test]
    fn structured_terms() {
        test_query(
            "teaches(professor(smith), course(math), room(a101)).",
            "teaches(professor(Prof), course(Subject), room(Room)).",
            &[("Prof", "smith"), ("Subject", "math"), ("Room", "a101")],
        );
    }

    #[test]
    fn single_variable_multiple_values() {
        test_query_variable_values(
            "parent(a, b). parent(a, c). parent(a, d).",
            "parent(a, X).",
            "X",
            &["b", "c", "d"],
        );
    }

    #[test]
    fn multiple_solutions_with_query_sequence() {
        let employee_program = r"works(john, company(google)). 
            works(mary, company(apple)). 
            works(bob, company(microsoft)).";
        test_query(
            employee_program,
            "works(john, company(Company)).",
            &[("Company", "google")],
        );
        test_query(
            employee_program,
            "works(mary, company(Company)).",
            &[("Company", "apple")],
        );
        test_query(
            employee_program,
            "works(bob, company(Company)).",
            &[("Company", "microsoft")],
        );
        test_query_count(employee_program, "works(Person, company(Company)).", 3);
    }
}
