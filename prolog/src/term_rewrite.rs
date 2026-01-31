use std::collections::HashMap;
use std::fmt;

use crate::parse::{Bound, Clause, Term, TermInner, list, range_var, struc, var};

/// 単一化エラー
#[derive(Debug, Clone)]
pub struct UnifyError {
    pub message: String,
    pub term1: Term,
    pub term2: Term,
}

impl fmt::Display for UnifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UnifyError {}

/// 書き換えステップのエラー
#[derive(Debug, Clone)]
pub struct RewriteError {
    pub message: String,
    pub goal: Term,
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.message, self.goal)
    }
}

impl std::error::Error for RewriteError {}

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
        TermInner::RangeVar { name, .. } => {
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
        TermInner::RangeVar { name, .. } => name == var_name,
        TermInner::Struct { args, .. } => args.iter().any(|arg| occurs_check(var_name, arg)),
        TermInner::List { items, tail } => {
            items.iter().any(|item| occurs_check(var_name, item))
                || tail.as_ref().map_or(false, |t| occurs_check(var_name, t))
        }
        TermInner::Number { .. } => false,
    }
}

/// 2つの下限境界から、より厳しい方を選択
fn intersect_min(a: Option<Bound>, b: Option<Bound>) -> Option<Bound> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(a), Some(b)) => {
            if a.value > b.value {
                Some(a)
            } else if b.value > a.value {
                Some(b)
            } else {
                // 同じ値の場合、exclusiveの方が厳しい
                Some(Bound {
                    value: a.value,
                    inclusive: a.inclusive && b.inclusive,
                })
            }
        }
    }
}

/// 2つの上限境界から、より厳しい方を選択
fn intersect_max(a: Option<Bound>, b: Option<Bound>) -> Option<Bound> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(a), Some(b)) => {
            if a.value < b.value {
                Some(a)
            } else if b.value < a.value {
                Some(b)
            } else {
                // 同じ値の場合、exclusiveの方が厳しい
                Some(Bound {
                    value: a.value,
                    inclusive: a.inclusive && b.inclusive,
                })
            }
        }
    }
}

/// 範囲が空でないかチェック（少なくとも1つの整数値が含まれるか）
fn range_is_valid(min: Option<Bound>, max: Option<Bound>) -> bool {
    match (min, max) {
        (Some(min), Some(max)) => {
            let effective_min = if min.inclusive {
                min.value
            } else {
                min.value + 1
            };
            let effective_max = if max.inclusive {
                max.value
            } else {
                max.value - 1
            };
            effective_min <= effective_max
        }
        _ => true,
    }
}

/// 値が範囲内にあるかチェック
fn value_in_range(value: i64, min: Option<Bound>, max: Option<Bound>) -> bool {
    let min_ok = match min {
        None => true,
        Some(b) if b.inclusive => value >= b.value,
        Some(b) => value > b.value,
    };
    let max_ok = match max {
        None => true,
        Some(b) if b.inclusive => value <= b.value,
        Some(b) => value < b.value,
    };
    min_ok && max_ok
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
        TermInner::RangeVar { name, .. } if name != "_" => {
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
pub fn unify(term1: &Term, term2: &Term) -> Result<Substitution, UnifyError> {
    let mut subst = Substitution::new();
    let mut stack = vec![(term1.clone(), term2.clone())];

    while let Some((t1, t2)) = stack.pop() {
        let t1 = deref_var(&t1, &subst);
        let t2 = deref_var(&t2, &subst);

        match (t1.as_ref(), t2.as_ref()) {
            // 同じ変数
            (TermInner::Var { name: n1 }, TermInner::Var { name: n2 }) if n1 == n2 => {}
            // RangeVar同士: 範囲の交差を計算
            (
                TermInner::RangeVar {
                    name: n1,
                    min: min1,
                    max: max1,
                },
                TermInner::RangeVar {
                    name: n2,
                    min: min2,
                    max: max2,
                },
            ) => {
                let new_min = intersect_min(*min1, *min2);
                let new_max = intersect_max(*max1, *max2);

                if !range_is_valid(new_min, new_max) {
                    return Err(UnifyError {
                        message: format!("range intersection is empty: {:?} ∩ {:?}", t1, t2),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
                }

                let n1 = n1.clone();
                let n2 = n2.clone();
                let intersected = range_var(n1.clone(), new_min, new_max);
                if n1 != "_" {
                    subst.insert(n1.clone(), intersected.clone());
                }
                if n2 != "_" && n2 != n1 {
                    subst.insert(n2, intersected);
                }
            }
            // RangeVarとNumber: 範囲内かチェック
            (TermInner::RangeVar { name, min, max }, TermInner::Number { value }) => {
                if !value_in_range(*value, *min, *max) {
                    return Err(UnifyError {
                        message: format!("value {} is out of range {:?}", value, t1),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
                }
                let name = name.clone();
                let t2 = t2.clone();
                if name != "_" {
                    subst.insert(name, t2);
                }
            }
            // swap して再処理
            (TermInner::Number { .. }, TermInner::RangeVar { .. }) => {
                stack.push((t2.clone(), t1.clone()));
            }
            // RangeVarと他 (Varと同様に扱う)
            (TermInner::RangeVar { name, .. }, _) if name != "_" => {
                if occurs_check(name, t2) {
                    return Err(UnifyError {
                        message: format!("occurs check failed: {} occurs in {:?}", name, t2),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
                }
                subst.insert(name.clone(), t2.clone());
            }
            (_, TermInner::RangeVar { name, .. }) if name != "_" => {
                stack.push((t2.clone(), t1.clone()));
            }
            // 変数と何か（anonymous変数 "_" は束縛しない）
            (TermInner::Var { name }, _) if name != "_" => {
                if occurs_check(name, t2) {
                    return Err(UnifyError {
                        message: format!("occurs check failed: {} occurs in {:?}", name, t2),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
                }
                subst.insert(name.clone(), t2.clone());
            }
            (_, TermInner::Var { name }) if name != "_" => {
                stack.push((t2.clone(), t1.clone()));
            }
            // anonymous変数はどんな項とも単一化成功（束縛なし）
            (TermInner::Var { name }, _) | (TermInner::RangeVar { name, .. }, _) if name == "_" => {
            }
            (_, TermInner::Var { name }) | (_, TermInner::RangeVar { name, .. }) if name == "_" => {
            }
            // 数値
            (TermInner::Number { value: v1 }, TermInner::Number { value: v2 }) => {
                if v1 != v2 {
                    return Err(UnifyError {
                        message: format!("number mismatch: {} != {}", v1, v2),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
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
                if f1 != f2 {
                    return Err(UnifyError {
                        message: format!("functor mismatch: {} != {}", f1, f2),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
                }
                if args1.len() != args2.len() {
                    return Err(UnifyError {
                        message: format!(
                            "arity mismatch: {}/{} != {}/{}",
                            f1,
                            args1.len(),
                            f2,
                            args2.len()
                        ),
                        term1: t1.clone(),
                        term2: t2.clone(),
                    });
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
                        return Err(UnifyError {
                            message: format!(
                                "list length mismatch: {} items vs {} items (no tail)",
                                items1.len(),
                                items2.len()
                            ),
                            term1: t1.clone(),
                            term2: t2.clone(),
                        });
                    }
                    (std::cmp::Ordering::Less, Some(t1), _) => {
                        let remaining: Vec<Term> = items2[min_len..].to_vec();
                        let new_list = list(remaining, tail2.clone());
                        stack.push((t1.clone(), new_list));
                    }
                    (std::cmp::Ordering::Less, None, _) => {
                        return Err(UnifyError {
                            message: format!(
                                "list length mismatch: {} items (no tail) vs {} items",
                                items1.len(),
                                items2.len()
                            ),
                            term1: t1.clone(),
                            term2: t2.clone(),
                        });
                    }
                }
            }
            _ => {
                return Err(UnifyError {
                    message: format!("cannot unify {:?} with {:?}", t1, t2),
                    term1: t1.clone(),
                    term2: t2.clone(),
                });
            }
        }
    }

    Ok(subst)
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
        TermInner::RangeVar { name, min, max } => {
            if name == "_" {
                range_var(name.clone(), *min, *max)
            } else {
                range_var(format!("{}_{}", name, suffix), *min, *max)
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

    fn rewrite_step(
        &mut self,
        goal: &Term,
    ) -> Result<(Clause, Substitution, Vec<Term>), RewriteError> {
        for clause in &self.db {
            self.clause_counter += 1;
            let renamed = rename_clause_vars(clause, &self.clause_counter.to_string());

            let (head, body) = match &renamed {
                Clause::Fact(term) => (term, &vec![]),
                Clause::Rule { head, body } => (head, body),
            };

            if let Ok(subst) = unify(goal, head) {
                let new_goals: Vec<Term> =
                    body.iter().map(|t| apply_substitution(t, &subst)).collect();
                return Ok((renamed, subst, new_goals));
            }
        }
        Err(RewriteError {
            message: "no clause matches goal".to_string(),
            goal: goal.clone(),
        })
    }

    pub fn execute_with_trace(
        &mut self,
        query: Vec<Term>,
    ) -> Result<(Substitution, Vec<Term>, Vec<TraceStep>), RewriteError> {
        let mut goals = query;
        let mut global_subst = Substitution::new();
        let mut trace = Vec::new();
        let mut resolved_goals = Vec::new();

        while let Some(goal) = goals.first().cloned() {
            goals.remove(0);

            let (matched_clause, subst, new_goals) = self.rewrite_step(&goal)?;
            global_subst = extend_substitution(&global_subst, &subst);

            let resolved = apply_substitution(&goal, &global_subst);
            resolved_goals.push(resolved);

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
        }

        Ok((global_subst, resolved_goals, trace))
    }

    pub fn execute(&mut self, query: Vec<Term>) -> Result<(Substitution, Vec<Term>), RewriteError> {
        let (subst, resolved, _) = self.execute_with_trace(query)?;
        Ok((subst, resolved))
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
        interp.execute(q).expect("Expected success").0
    }

    fn run_success_with_trace(db_src: &str, query_src: &str) -> (Substitution, Vec<TraceStep>) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        let (subst, _, trace) = interp.execute_with_trace(q).expect("Expected success");
        (subst, trace)
    }

    fn run_success_with_rewritten(db_src: &str, query_src: &str) -> (Substitution, Vec<Term>) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        let (subst, rewritten) = interp.execute(q).expect("Expected success");
        (subst, rewritten)
    }

    fn run_failure(db_src: &str, query_src: &str) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        assert!(interp.execute(q).is_err(), "Expected failure, got success");
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
        assert!(unify(&t1, &t2).is_err());
    }

    // ===== RangeVar unify tests =====

    #[test]
    fn test_rangevar_number_in_range() {
        use crate::parse::{Bound, number, range_var};
        let rv = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: false,
            }),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
        );
        let n = number(5);
        let result = unify(&rv, &n).unwrap();
        assert_eq!(result.get("X").unwrap(), &n);
    }

    #[test]
    fn test_rangevar_number_out_of_range() {
        use crate::parse::{Bound, number, range_var};
        let rv = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: false,
            }),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
        );
        let n = number(15);
        assert!(unify(&rv, &n).is_err());
    }

    #[test]
    fn test_rangevar_number_boundary_exclusive() {
        use crate::parse::{Bound, number, range_var};
        let rv = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: false,
            }),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
        );
        // 0 is excluded (0 < X)
        assert!(unify(&rv, &number(0)).is_err());
        // 10 is excluded (X < 10)
        assert!(unify(&rv, &number(10)).is_err());
        // 1 and 9 are included
        assert!(unify(&rv, &number(1)).is_ok());
        assert!(unify(&rv, &number(9)).is_ok());
    }

    #[test]
    fn test_rangevar_number_boundary_inclusive() {
        use crate::parse::{Bound, number, range_var};
        let rv = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: true,
            }),
            Some(Bound {
                value: 10,
                inclusive: true,
            }),
        );
        // 0 and 10 are included
        assert!(unify(&rv, &number(0)).is_ok());
        assert!(unify(&rv, &number(10)).is_ok());
    }

    #[test]
    fn test_rangevar_intersection() {
        use crate::parse::{Bound, range_var};
        // 0 < X < 10
        let rv1 = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: false,
            }),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
        );
        // 5 < Y < 15
        let rv2 = range_var(
            "Y".to_string(),
            Some(Bound {
                value: 5,
                inclusive: false,
            }),
            Some(Bound {
                value: 15,
                inclusive: false,
            }),
        );
        let result = unify(&rv1, &rv2).unwrap();
        // intersection: 5 < Z < 10
        let x_term = result.get("X").unwrap();
        match x_term.as_ref() {
            TermInner::RangeVar { min, max, .. } => {
                assert_eq!(
                    *min,
                    Some(Bound {
                        value: 5,
                        inclusive: false
                    })
                );
                assert_eq!(
                    *max,
                    Some(Bound {
                        value: 10,
                        inclusive: false
                    })
                );
            }
            _ => panic!("Expected RangeVar"),
        }
    }

    #[test]
    fn test_rangevar_intersection_empty() {
        use crate::parse::{Bound, range_var};
        // 0 < X < 5
        let rv1 = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: false,
            }),
            Some(Bound {
                value: 5,
                inclusive: false,
            }),
        );
        // 10 < Y < 15
        let rv2 = range_var(
            "Y".to_string(),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
            Some(Bound {
                value: 15,
                inclusive: false,
            }),
        );
        assert!(unify(&rv1, &rv2).is_err());
    }

    #[test]
    fn test_rangevar_intersection_inclusive_exclusive() {
        use crate::parse::{Bound, range_var};
        // 0 <= X <= 5
        let rv1 = range_var(
            "X".to_string(),
            Some(Bound {
                value: 0,
                inclusive: true,
            }),
            Some(Bound {
                value: 5,
                inclusive: true,
            }),
        );
        // 5 < Y < 10
        let rv2 = range_var(
            "Y".to_string(),
            Some(Bound {
                value: 5,
                inclusive: false,
            }),
            Some(Bound {
                value: 10,
                inclusive: false,
            }),
        );
        // intersection: 5 < Z <= 5 which is empty
        assert!(unify(&rv1, &rv2).is_err());
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
        assert_eq!(
            x_term,
            struc("b".to_string(), vec![struc("c".to_string(), vec![])])
        );
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
    fn resolved_goals_returned() {
        let (_, resolved) =
            run_success_with_rewritten("p :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p.");
        // p, q(a, b), r(b, c) の3つが解決される
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0], struc("p".to_string(), vec![]));
        assert_eq!(
            resolved[1],
            struc(
                "q".to_string(),
                vec![
                    struc("a".to_string(), vec![]),
                    struc("b".to_string(), vec![]),
                ]
            )
        );
        assert_eq!(
            resolved[2],
            struc(
                "r".to_string(),
                vec![
                    struc("b".to_string(), vec![]),
                    struc("c".to_string(), vec![]),
                ]
            )
        );
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
            struc(
                "pair".to_string(),
                vec![
                    struc("a".to_string(), vec![]),
                    struc("b".to_string(), vec![])
                ]
            )
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
            struc(
                "pair".to_string(),
                vec![
                    struc("a".to_string(), vec![]),
                    struc("b".to_string(), vec![])
                ]
            )
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

