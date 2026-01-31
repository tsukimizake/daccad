use std::collections::HashMap;

use crate::parse::{Clause, Term, TermInner, list, struc, var};

/// 変数名から項への代入
pub type Substitution = HashMap<String, Term>;

/// 代入を項に適用する
pub fn apply_substitution(term: &Term, subst: &Substitution) -> Term {
    match term.as_ref() {
        TermInner::Var { name } => {
            if let Some(value) = subst.get(name) {
                apply_substitution(value, subst)
            } else {
                term.clone()
            }
        }
        TermInner::Struct { functor, args } => {
            let new_args = args
                .iter()
                .map(|arg| apply_substitution(arg, subst))
                .collect();
            struc(functor.clone(), new_args)
        }
        TermInner::List { items, tail } => {
            let new_items = items
                .iter()
                .map(|item| apply_substitution(item, subst))
                .collect();
            let new_tail = tail.as_ref().map(|t| apply_substitution(t, subst));
            list(new_items, new_tail)
        }
        TermInner::Number { .. } => term.clone(),
    }
}

/// 2つの代入をマージする（subst2をsubst1に追加）
fn extend_substitution(subst1: &Substitution, subst2: &Substitution) -> Substitution {
    let mut result = subst1.clone();
    for (k, v) in subst2 {
        result.entry(k.clone()).or_insert_with(|| v.clone());
    }
    result
}

/// occurs check: 変数varが項term内に出現するか
fn occurs_check(var_name: &str, term: &Term) -> bool {
    match term.as_ref() {
        TermInner::Var { name } => name == var_name,
        TermInner::Struct { args, .. } => args.iter().any(|arg| occurs_check(var_name, arg)),
        TermInner::List { items, tail } => {
            items.iter().any(|item| occurs_check(var_name, item))
                || tail.as_ref().map_or(false, |t| occurs_check(var_name, t))
        }
        TermInner::Number { .. } => false,
    }
}

/// 変数を代入で解決する（連鎖をたどる）
fn deref_var<'a>(term: &'a Term, subst: &'a Substitution) -> &'a Term {
    match term.as_ref() {
        TermInner::Var { name } if name != "_" => {
            if let Some(bound) = subst.get(name) {
                deref_var(bound, subst)
            } else {
                term
            }
        }
        _ => term,
    }
}

/// 2つの項を単一化し、成功すれば代入を返す
pub fn unify(term1: &Term, term2: &Term) -> Option<Substitution> {
    let mut subst = Substitution::new();
    let mut stack = vec![(term1.clone(), term2.clone())];

    while let Some((t1, t2)) = stack.pop() {
        let t1 = deref_var(&t1, &subst);
        let t2 = deref_var(&t2, &subst);

        match (t1.as_ref(), t2.as_ref()) {
            // 同じ変数
            (TermInner::Var { name: n1 }, TermInner::Var { name: n2 }) if n1 == n2 => {}
            // 変数と何か（anonymous変数 "_" は束縛しない）
            (TermInner::Var { name }, _) if name != "_" => {
                if occurs_check(name, t2) {
                    return None;
                }
                subst.insert(name.clone(), t2.clone());
            }
            (_, TermInner::Var { name }) if name != "_" => {
                if occurs_check(name, t1) {
                    return None;
                }
                subst.insert(name.clone(), t1.clone());
            }
            // anonymous変数はどんな項とも単一化成功（束縛なし）
            (TermInner::Var { name }, _) if name == "_" => {}
            (_, TermInner::Var { name }) if name == "_" => {}
            // 数値
            (TermInner::Number { value: v1 }, TermInner::Number { value: v2 }) => {
                if v1 != v2 {
                    return None;
                }
            }
            // 構造体
            (
                TermInner::Struct {
                    functor: f1,
                    args: args1,
                },
                TermInner::Struct {
                    functor: f2,
                    args: args2,
                },
            ) => {
                if f1 != f2 || args1.len() != args2.len() {
                    return None;
                }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    stack.push((a1.clone(), a2.clone()));
                }
            }
            // リスト
            (
                TermInner::List {
                    items: items1,
                    tail: tail1,
                },
                TermInner::List {
                    items: items2,
                    tail: tail2,
                },
            ) => {
                let min_len = items1.len().min(items2.len());
                for i in 0..min_len {
                    stack.push((items1[i].clone(), items2[i].clone()));
                }

                match (items1.len().cmp(&items2.len()), tail1, tail2) {
                    (std::cmp::Ordering::Equal, Some(t1), Some(t2)) => {
                        stack.push((t1.clone(), t2.clone()));
                    }
                    (std::cmp::Ordering::Equal, None, None) => {}
                    (std::cmp::Ordering::Equal, Some(t1), None) => {
                        stack.push((t1.clone(), list(vec![], None)));
                    }
                    (std::cmp::Ordering::Equal, None, Some(t2)) => {
                        stack.push((list(vec![], None), t2.clone()));
                    }
                    (std::cmp::Ordering::Greater, _, Some(t2)) => {
                        let remaining: Vec<Term> = items1[min_len..].to_vec();
                        let new_list = list(remaining, tail1.clone());
                        stack.push((new_list, t2.clone()));
                    }
                    (std::cmp::Ordering::Greater, _, None) => {
                        return None;
                    }
                    (std::cmp::Ordering::Less, Some(t1), _) => {
                        let remaining: Vec<Term> = items2[min_len..].to_vec();
                        let new_list = list(remaining, tail2.clone());
                        stack.push((t1.clone(), new_list));
                    }
                    (std::cmp::Ordering::Less, None, _) => {
                        return None;
                    }
                }
            }
            _ => return None,
        }
    }

    Some(subst)
}

/// 節の変数をリネームして衝突を避ける
fn rename_clause_vars(clause: &Clause, suffix: &str) -> Clause {
    match clause {
        Clause::Fact(term) => Clause::Fact(rename_term_vars(term, suffix)),
        Clause::Rule { head, body } => Clause::Rule {
            head: rename_term_vars(head, suffix),
            body: body.iter().map(|t| rename_term_vars(t, suffix)).collect(),
        },
    }
}

fn rename_term_vars(term: &Term, suffix: &str) -> Term {
    match term.as_ref() {
        TermInner::Var { name } => {
            if name == "_" {
                var(name.clone())
            } else {
                var(format!("{}_{}", name, suffix))
            }
        }
        TermInner::Struct { functor, args } => {
            let new_args = args.iter().map(|a| rename_term_vars(a, suffix)).collect();
            struc(functor.clone(), new_args)
        }
        TermInner::List { items, tail } => {
            let new_items = items.iter().map(|i| rename_term_vars(i, suffix)).collect();
            let new_tail = tail.as_ref().map(|t| rename_term_vars(t, suffix));
            list(new_items, new_tail)
        }
        TermInner::Number { .. } => term.clone(),
    }
}

/// 実行トレースの1ステップ
#[derive(Debug, Clone)]
pub struct TraceStep {
    pub selected_goal: Term,
    pub matched_clause: Clause,
    pub substitution: Substitution,
    pub new_goals: Vec<Term>,
}

/// インタプリタの実行結果
#[derive(Debug)]
pub enum ExecutionResult {
    Success(Substitution),
    Failure,
}

/// Term rewrite方式のインタプリタ
pub struct Interpreter {
    db: Vec<Clause>,
    clause_counter: usize,
}

impl Interpreter {
    pub fn new(db: Vec<Clause>) -> Self {
        Self {
            db,
            clause_counter: 0,
        }
    }

    fn rewrite_step(&mut self, goal: &Term) -> Option<(Clause, Substitution, Vec<Term>)> {
        for clause in &self.db {
            self.clause_counter += 1;
            let renamed = rename_clause_vars(clause, &self.clause_counter.to_string());

            let (head, body) = match &renamed {
                Clause::Fact(term) => (term, vec![]),
                Clause::Rule { head, body } => (head, body.clone()),
            };

            if let Some(subst) = unify(goal, head) {
                let new_goals: Vec<Term> =
                    body.iter().map(|t| apply_substitution(t, &subst)).collect();
                return Some((renamed, subst, new_goals));
            }
        }
        None
    }

    pub fn execute_with_trace(&mut self, query: Vec<Term>) -> (ExecutionResult, Vec<TraceStep>) {
        let mut goals = query;
        let mut global_subst = Substitution::new();
        let mut trace = Vec::new();

        while let Some(goal) = goals.first().cloned() {
            goals.remove(0);

            if let Some((matched_clause, subst, new_goals)) = self.rewrite_step(&goal) {
                global_subst = extend_substitution(&global_subst, &subst);

                trace.push(TraceStep {
                    selected_goal: goal,
                    matched_clause: matched_clause.clone(),
                    substitution: subst.clone(),
                    new_goals: new_goals.clone(),
                });

                goals = new_goals
                    .into_iter()
                    .chain(goals)
                    .map(|g| apply_substitution(&g, &subst))
                    .collect();
            } else {
                return (ExecutionResult::Failure, trace);
            }
        }

        (ExecutionResult::Success(global_subst), trace)
    }

    pub fn execute(&mut self, query: Vec<Term>) -> ExecutionResult {
        self.execute_with_trace(query).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{database, query};

    fn run_success(db_src: &str, query_src: &str) -> Substitution {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        match interp.execute(q) {
            ExecutionResult::Success(subst) => subst,
            ExecutionResult::Failure => panic!("Expected success, got failure"),
        }
    }

    fn run_success_with_trace(db_src: &str, query_src: &str) -> (Substitution, Vec<TraceStep>) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        match interp.execute_with_trace(q) {
            (ExecutionResult::Success(subst), trace) => (subst, trace),
            (ExecutionResult::Failure, _) => panic!("Expected success, got failure"),
        }
    }

    fn run_failure(db_src: &str, query_src: &str) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        match interp.execute(q) {
            ExecutionResult::Success(_) => panic!("Expected failure, got success"),
            ExecutionResult::Failure => {}
        }
    }

    fn get_atom(subst: &Substitution, var_name: &str) -> String {
        let term = subst
            .get(var_name)
            .unwrap_or_else(|| panic!("Variable {} not found", var_name));
        let term = apply_substitution(term, subst);
        match term.as_ref() {
            TermInner::Struct { functor, args } if args.is_empty() => functor.clone(),
            _ => panic!("Expected atom, got {:?}", term),
        }
    }

    fn get_term(subst: &Substitution, var_name: &str) -> Term {
        let term = subst
            .get(var_name)
            .unwrap_or_else(|| panic!("Variable {} not found", var_name));
        apply_substitution(term, subst)
    }

    // ===== unify tests =====

    #[test]
    fn test_unify_vars() {
        let x = var("X".to_string());
        let a = struc("a".to_string(), vec![]);
        let result = unify(&x, &a).unwrap();
        assert_eq!(result.get("X").unwrap(), &a);
    }

    #[test]
    fn test_unify_structs() {
        let t1 = struc("f".to_string(), vec![var("X".to_string())]);
        let t2 = struc("f".to_string(), vec![struc("a".to_string(), vec![])]);
        let result = unify(&t1, &t2).unwrap();
        assert_eq!(result.get("X").unwrap(), &struc("a".to_string(), vec![]));
    }

    #[test]
    fn test_unify_fail() {
        let t1 = struc("f".to_string(), vec![]);
        let t2 = struc("g".to_string(), vec![]);
        assert!(unify(&t1, &t2).is_none());
    }

    // ===== basic fact tests =====

    #[test]
    fn simple_atom_match() {
        run_success("hello.", "hello.");
    }

    #[test]
    fn fail_unmatched() {
        run_failure("hello.", "bye.");
    }

    #[test]
    fn db_var_matches_constant_query() {
        run_success("honi(X).", "honi(fuwa).");
    }

    #[test]
    fn query_var_binds_to_constant_fact() {
        let subst = run_success("honi(fuwa).", "honi(X).");
        assert_eq!(get_atom(&subst, "X"), "fuwa");
    }

    #[test]
    fn var_to_var_binding() {
        let subst = run_success("honi(X).", "honi(Y).");
        let y_term = get_term(&subst, "Y");
        assert!(matches!(y_term.as_ref(), TermInner::Var { .. }));
    }

    #[test]
    fn multiple_usages_of_same_variable() {
        let subst = run_success("likes(X, X).", "likes(fuwa, Y).");
        assert_eq!(get_atom(&subst, "Y"), "fuwa");
    }

    // ===== nested struct tests =====

    #[test]
    fn deep_struct_on_db() {
        let subst = run_success("a(b(c)).", "a(X).");
        let x_term = get_term(&subst, "X");
        assert_eq!(x_term, struc("b".to_string(), vec![struc("c".to_string(), vec![])]));
    }

    #[test]
    fn deep_struct_on_query() {
        run_success("a(X).", "a(b(c)).");
    }

    #[test]
    fn recursive_unify_nested_struct_match() {
        run_success("f(a(b)).", "f(a(b)).");
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_inner() {
        run_failure("f(a(b)).", "f(a(c)).");
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_functor() {
        run_failure("f(a(b)).", "f(c(b)).");
    }

    #[test]
    fn recursive_unify_var_in_nested_struct() {
        run_success("f(a(X)).", "f(a(b)).");
    }

    #[test]
    fn recursive_unify_query_var_binds_in_nested() {
        let subst = run_success("f(a(b)).", "f(a(X)).");
        assert_eq!(get_atom(&subst, "X"), "b");
    }

    #[test]
    fn recursive_unify_multiple_args() {
        run_success("f(a(b), c(d)).", "f(a(b), c(d)).");
    }

    #[test]
    fn recursive_unify_multiple_args_one_mismatch() {
        run_failure("f(a(b), c(d)).", "f(a(b), c(e)).");
    }

    #[test]
    fn recursive_unify_three_levels_deep() {
        run_success("f(a(b(c))).", "f(a(b(c))).");
    }

    #[test]
    fn recursive_unify_three_levels_deep_mismatch() {
        run_failure("f(a(b(c))).", "f(a(b(d))).");
    }

    #[test]
    fn recursive_unify_var_at_deep_level() {
        run_success("f(a(b(X))).", "f(a(b(c))).");
    }

    // ===== rule tests =====

    #[test]
    fn simpler_rule() {
        run_success("p :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p.");
    }

    #[test]
    fn sample_rule() {
        let subst = run_success("p(X,Y) :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p(A, B).");
        assert_eq!(get_atom(&subst, "A"), "a");
        assert_eq!(get_atom(&subst, "B"), "c");
    }

    #[test]
    fn rule_single_goal() {
        run_success("parent(X) :- father(X). father(tom).", "parent(tom).");
    }

    #[test]
    fn rule_single_goal_with_var_query() {
        let subst = run_success("parent(X) :- father(X). father(tom).", "parent(Y).");
        assert_eq!(get_atom(&subst, "Y"), "tom");
    }

    #[test]
    fn grandparent_rule() {
        let db = r#"
            parent(alice, bob).
            parent(bob, carol).
            grandparent(X, Y) :- parent(X, Z), parent(Z, Y).
        "#;
        let subst = run_success(db, "grandparent(alice, Who).");
        assert_eq!(get_atom(&subst, "Who"), "carol");
    }

    // ===== list tests =====

    #[test]
    fn list_empty_match() {
        run_success("f([]).", "f([]).");
    }

    #[test]
    fn list_simple_match() {
        run_success("f([a, b, c]).", "f([a, b, c]).");
    }

    #[test]
    fn list_mismatch() {
        run_failure("f([a, b]).", "f([a, c]).");
    }

    #[test]
    fn list_var_binding() {
        let subst = run_success("f([a, b, c]).", "f(X).");
        let x_term = get_term(&subst, "X");
        assert!(matches!(x_term.as_ref(), TermInner::List { .. }));
    }

    #[test]
    fn list_head_tail_pattern() {
        let subst = run_success("f([a, b, c]).", "f([H|T]).");
        assert_eq!(get_atom(&subst, "H"), "a");
    }

    #[test]
    fn member_first() {
        let db = "member(X, [X|_]).";
        run_success(db, "member(a, [a, b, c]).");
    }

    // ===== trace tests =====

    #[test]
    fn trace_records_rewrite() {
        let db = "q(a, b). p(X, Y) :- q(X, Y).";
        let (subst, trace) = run_success_with_trace(db, "p(W, V).");

        assert_eq!(trace.len(), 2);
        assert!(matches!(&trace[0].matched_clause, Clause::Rule { .. }));
        assert!(matches!(&trace[1].matched_clause, Clause::Fact(_)));

        assert_eq!(get_atom(&subst, "W"), "a");
        assert_eq!(get_atom(&subst, "V"), "b");
    }

    // ===== rule failure tests =====

    #[test]
    fn rule_fails_first_subgoal() {
        run_failure("p(X) :- q(X), r(X). q(b). r(a).", "p(a).");
    }

    #[test]
    fn rule_fails_second_subgoal() {
        run_failure("p(X) :- q(X), r(X). q(a). r(b).", "p(a).");
    }

    #[test]
    fn rule_fails_no_matching_fact() {
        run_failure("p(X) :- q(X). q(a).", "p(b).");
    }

    // ===== rule chain tests =====

    #[test]
    fn rule_chain_two_levels() {
        run_success("a(X) :- b(X). b(X) :- c(X). c(foo).", "a(foo).");
    }

    #[test]
    fn rule_chain_three_levels() {
        run_success(
            "a(X) :- b(X). b(X) :- c(X). c(X) :- d(X). d(bar).",
            "a(bar).",
        );
    }

    #[test]
    fn rule_chain_with_var_binding() {
        let subst = run_success("a(X) :- b(X). b(X) :- c(X). c(baz).", "a(Y).");
        assert_eq!(get_atom(&subst, "Y"), "baz");
    }

    // ===== rule with nested struct tests =====

    #[test]
    fn rule_with_nested_struct_in_fact() {
        run_success(
            "outer(X) :- inner(X). inner(pair(a, b)).",
            "outer(pair(a, b)).",
        );
    }

    #[test]
    fn rule_with_nested_struct_var_binding() {
        let subst = run_success("outer(X) :- inner(X). inner(pair(a, b)).", "outer(Y).");
        let y_term = get_term(&subst, "Y");
        assert_eq!(
            y_term,
            struc("pair".to_string(), vec![struc("a".to_string(), vec![]), struc("b".to_string(), vec![])])
        );
    }

    #[test]
    fn rule_with_deeply_nested_struct() {
        run_success(
            "wrap(X) :- data(X). data(node(leaf(a), leaf(b))).",
            "wrap(node(leaf(a), leaf(b))).",
        );
    }

    #[test]
    fn rule_shared_variable_in_body() {
        run_success("same(X) :- eq(X, X). eq(a, a).", "same(a).");
    }

    // ===== rule with multiple args =====

    #[test]
    fn rule_three_args() {
        let subst = run_success(
            "triple(X, Y, Z) :- first(X), second(Y), third(Z). first(a). second(b). third(c).",
            "triple(A, B, C).",
        );
        assert_eq!(get_atom(&subst, "A"), "a");
        assert_eq!(get_atom(&subst, "B"), "b");
        assert_eq!(get_atom(&subst, "C"), "c");
    }

    // ===== rule head with struct =====

    #[test]
    fn rule_head_with_struct() {
        run_success(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(pair(a, b)).",
        );
    }

    #[test]
    fn rule_head_with_struct_var_query() {
        let subst = run_success(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(P).",
        );
        let p_term = get_term(&subst, "P");
        assert_eq!(
            p_term,
            struc("pair".to_string(), vec![struc("a".to_string(), vec![]), struc("b".to_string(), vec![])])
        );
    }

    // ===== backtracking required (ignored for now) =====

    #[test]
    #[ignore]
    fn rule_multiple_goals() {
        run_success(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, c).",
        );
    }

    #[test]
    #[ignore]
    fn rule_multiple_goals_with_var() {
        let subst = run_success(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, W).",
        );
        assert_eq!(get_atom(&subst, "W"), "c");
    }

    #[test]
    #[ignore]
    fn rule_shared_variable_propagation() {
        let subst = run_success(
            "connect(X, Z) :- link(X, Y), link(Y, Z). link(a, b). link(b, c).",
            "connect(a, Z).",
        );
        assert_eq!(get_atom(&subst, "Z"), "c");
    }

    #[test]
    #[ignore]
    fn rule_mixed_with_facts() {
        run_success(
            "animal(dog). animal(cat). is_pet(X) :- animal(X).",
            "is_pet(dog).",
        );
    }
}
