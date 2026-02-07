use std::fmt;

use crate::constraint::{SolveResult, solve_arithmetic};
use crate::parse::{ArithOp, Bound, Clause, Term, arith_expr, list, number, range_var, struc, var};

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

/// 単一変数の置換をインプレースで適用
fn substitute_in_place(term: &mut Term, var_name: &str, replacement: &Term) {
    match term {
        Term::Var { name } if name == var_name => {
            *term = replacement.clone();
        }
        Term::RangeVar { name, .. } if name == var_name => {
            *term = replacement.clone();
        }
        Term::Struct { args, .. } => {
            for arg in args.iter_mut() {
                substitute_in_place(arg, var_name, replacement);
            }
        }
        Term::List { items, tail } => {
            for item in items.iter_mut() {
                substitute_in_place(item, var_name, replacement);
            }
            if let Some(t) = tail {
                substitute_in_place(t.as_mut(), var_name, replacement);
            }
        }
        Term::ArithExpr { left, right, .. } => {
            substitute_in_place(left.as_mut(), var_name, replacement);
            substitute_in_place(right.as_mut(), var_name, replacement);
        }
        _ => {}
    }
}

/// unify 内のスタックに対して変数置換を適用
fn substitute_in_stack(stack: &mut Vec<(Term, Term)>, var_name: &str, replacement: &Term) {
    for (t1, t2) in stack.iter_mut() {
        substitute_in_place(t1, var_name, replacement);
        substitute_in_place(t2, var_name, replacement);
    }
}

/// goals に対して変数置換を適用
fn substitute_in_goals(goals: &mut Vec<Term>, var_name: &str, replacement: &Term) {
    for goal in goals.iter_mut() {
        substitute_in_place(goal, var_name, replacement);
    }
}

/// 算術式を評価する。評価できない場合（未束縛変数を含む場合）はNoneを返す
fn eval_arith(term: &Term) -> Option<i64> {
    match term {
        Term::Number { value } => Some(*value),
        Term::ArithExpr { op, left, right } => {
            let l = eval_arith(left)?;
            let r = eval_arith(right)?;
            Some(match op {
                ArithOp::Add => l + r,
                ArithOp::Sub => l - r,
                ArithOp::Mul => l * r,
                ArithOp::Div => l / r,
            })
        }
        _ => None,
    }
}

/// 算術式をインプレースで評価し、可能なら数値に置き換える
fn eval_arith_in_place(term: &mut Term) {
    if let Some(val) = eval_arith(term) {
        *term = number(val);
    } else {
        // 再帰的に子要素も評価
        match term {
            Term::ArithExpr { left, right, .. } => {
                eval_arith_in_place(left.as_mut());
                eval_arith_in_place(right.as_mut());
            }
            Term::Struct { args, .. } => {
                for arg in args.iter_mut() {
                    eval_arith_in_place(arg);
                }
            }
            Term::List { items, tail } => {
                for item in items.iter_mut() {
                    eval_arith_in_place(item);
                }
                if let Some(t) = tail {
                    eval_arith_in_place(t.as_mut());
                }
            }
            _ => {}
        }
    }
}

/// occurs check: 変数varが項term内に出現するか
fn occurs_check(var_name: &str, term: &Term) -> bool {
    match term {
        Term::Var { name } => name == var_name,
        Term::RangeVar { name, .. } => name == var_name,
        Term::Struct { args, .. } => args.iter().any(|arg| occurs_check(var_name, arg)),
        Term::List { items, tail } => {
            items.iter().any(|item| occurs_check(var_name, item))
                || tail.as_ref().map_or(false, |t| occurs_check(var_name, t))
        }
        Term::ArithExpr { left, right, .. } => {
            occurs_check(var_name, left) || occurs_check(var_name, right)
        }
        Term::Number { .. } => false,
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

/// deferred リスト内の全Termに対して変数を置換する
fn substitute_in_deferred(deferred: &mut Vec<(Term, Term)>, var_name: &str, replacement: &Term) {
    for (t1, t2) in deferred.iter_mut() {
        substitute_in_place(t1, var_name, replacement);
        substitute_in_place(t2, var_name, replacement);
    }
}

/// 2つの項を単一化し、成功すれば goals をインプレースで書き換える
pub fn unify(term1: Term, term2: Term, goals: &mut Vec<Term>) -> Result<(), UnifyError> {
    let mut stack = vec![(term1, term2)];
    // 未評価の算術式制約を保持（変数が束縛されたら再評価）
    let mut deferred: Vec<(Term, Term)> = Vec::new();

    while let Some((mut t1, mut t2)) = stack.pop() {
        // 算術式を評価可能なら数値に変換
        if let Some(val) = eval_arith(&t1) {
            t1 = number(val);
        }
        if let Some(val) = eval_arith(&t2) {
            t2 = number(val);
        }

        // 算術式がまだ評価できない場合は遅延
        let has_unbound_var = |term: &Term| -> bool {
            matches!(term, Term::ArithExpr { .. })
        };

        if has_unbound_var(&t1) || has_unbound_var(&t2) {
            deferred.push((t1, t2));
            continue;
        }

        match (&t1, &t2) {
            // 同じ変数
            (Term::Var { name: n1 }, Term::Var { name: n2 }) if n1 == n2 => {}
            // RangeVar同士: 範囲の交差を計算
            (
                Term::RangeVar {
                    name: n1,
                    min: min1,
                    max: max1,
                },
                Term::RangeVar {
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
                        term1: t1,
                        term2: t2,
                    });
                }

                let intersected = range_var(n1.clone(), new_min, new_max);
                if n1 != "_" {
                    substitute_in_stack(&mut stack, n1, &intersected);
                    substitute_in_goals(goals, n1, &intersected);
                    substitute_in_deferred(&mut deferred, n1, &intersected);
                }
                if n2 != "_" && n2 != n1 {
                    substitute_in_stack(&mut stack, n2, &intersected);
                    substitute_in_goals(goals, n2, &intersected);
                    substitute_in_deferred(&mut deferred, n2, &intersected);
                }
            }
            // RangeVarとNumber: 範囲内かチェック
            (Term::RangeVar { name, min, max }, Term::Number { value }) => {
                if !value_in_range(*value, *min, *max) {
                    return Err(UnifyError {
                        message: format!("value {} is out of range {:?}", value, t1),
                        term1: t1,
                        term2: t2,
                    });
                }
                if name != "_" {
                    substitute_in_stack(&mut stack, name, &t2);
                    substitute_in_goals(goals, name, &t2);
                    substitute_in_deferred(&mut deferred, name, &t2);
                }
            }
            // swap して再処理
            (Term::Number { .. }, Term::RangeVar { .. }) => {
                stack.push((t2, t1));
            }
            // RangeVarと他 (Varと同様に扱う)
            (Term::RangeVar { name, .. }, _) if name != "_" => {
                if occurs_check(name, &t2) {
                    return Err(UnifyError {
                        message: format!("occurs check failed: {} occurs in {:?}", name, t2),
                        term1: t1,
                        term2: t2,
                    });
                }
                substitute_in_stack(&mut stack, name, &t2);
                substitute_in_goals(goals, name, &t2);
                substitute_in_deferred(&mut deferred, name, &t2);
            }
            (_, Term::RangeVar { name, .. }) if name != "_" => {
                stack.push((t2, t1));
            }
            // 変数と何か（anonymous変数 "_" は束縛しない）
            (Term::Var { name }, _) if name != "_" => {
                if occurs_check(name, &t2) {
                    return Err(UnifyError {
                        message: format!("occurs check failed: {} occurs in {:?}", name, t2),
                        term1: t1,
                        term2: t2,
                    });
                }
                substitute_in_stack(&mut stack, name, &t2);
                substitute_in_goals(goals, name, &t2);
                substitute_in_deferred(&mut deferred, name, &t2);
            }
            (_, Term::Var { name }) if name != "_" => {
                stack.push((t2, t1));
            }
            // anonymous変数はどんな項とも単一化成功（束縛なし）
            (Term::Var { name }, _) | (Term::RangeVar { name, .. }, _) if name == "_" => {}
            (_, Term::Var { name }) | (_, Term::RangeVar { name, .. }) if name == "_" => {}
            // 数値
            (Term::Number { value: v1 }, Term::Number { value: v2 }) => {
                if v1 != v2 {
                    return Err(UnifyError {
                        message: format!("number mismatch: {} != {}", v1, v2),
                        term1: t1,
                        term2: t2,
                    });
                }
            }
            // 構造体
            (
                Term::Struct {
                    functor: f1,
                    args: args1,
                },
                Term::Struct {
                    functor: f2,
                    args: args2,
                },
            ) => {
                if f1 != f2 {
                    return Err(UnifyError {
                        message: format!("functor mismatch: {} != {}", f1, f2),
                        term1: t1,
                        term2: t2,
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
                        term1: t1,
                        term2: t2,
                    });
                }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    stack.push((a1.clone(), a2.clone()));
                }
            }
            // リスト
            (
                Term::List {
                    items: items1,
                    tail: tail1,
                },
                Term::List {
                    items: items2,
                    tail: tail2,
                },
            ) => {
                let min_len = items1.len().min(items2.len());
                // 要素を逆順でpushしてLIFOでも左から右の順序で処理されるようにする
                for i in (0..min_len).rev() {
                    stack.push((items1[i].clone(), items2[i].clone()));
                }

                match (items1.len().cmp(&items2.len()), tail1, tail2) {
                    (std::cmp::Ordering::Equal, Some(t1), Some(t2)) => {
                        stack.push((t1.as_ref().clone(), t2.as_ref().clone()));
                    }
                    (std::cmp::Ordering::Equal, None, None) => {}
                    (std::cmp::Ordering::Equal, Some(t1), None) => {
                        stack.push((t1.as_ref().clone(), list(vec![], None)));
                    }
                    (std::cmp::Ordering::Equal, None, Some(t2)) => {
                        stack.push((list(vec![], None), t2.as_ref().clone()));
                    }
                    (std::cmp::Ordering::Greater, _, Some(t2_tail)) => {
                        let remaining: Vec<Term> = items1[min_len..].to_vec();
                        let new_list = list(remaining, tail1.as_ref().map(|t| t.as_ref().clone()));
                        stack.push((new_list, t2_tail.as_ref().clone()));
                    }
                    (std::cmp::Ordering::Greater, _, None) => {
                        return Err(UnifyError {
                            message: format!(
                                "list length mismatch: {} items vs {} items (no tail)",
                                items1.len(),
                                items2.len()
                            ),
                            term1: t1,
                            term2: t2,
                        });
                    }
                    (std::cmp::Ordering::Less, Some(t1_tail), _) => {
                        let remaining: Vec<Term> = items2[min_len..].to_vec();
                        let new_list = list(remaining, tail2.as_ref().map(|t| t.as_ref().clone()));
                        stack.push((t1_tail.as_ref().clone(), new_list));
                    }
                    (std::cmp::Ordering::Less, None, _) => {
                        return Err(UnifyError {
                            message: format!(
                                "list length mismatch: {} items (no tail) vs {} items",
                                items1.len(),
                                items2.len()
                            ),
                            term1: t1,
                            term2: t2,
                        });
                    }
                }
            }
            _ => {
                return Err(UnifyError {
                    message: format!("cannot unify {:?} with {:?}", t1, t2),
                    term1: t1,
                    term2: t2,
                });
            }
        }

        // 変数が束縛されたので、遅延された制約を再評価
        if !deferred.is_empty() {
            let old_deferred = std::mem::take(&mut deferred);
            for constraint in old_deferred {
                stack.push(constraint);
            }
        }
    }

    // 最後に残った遅延制約を制約ソルバーで処理
    for (t1, t2) in deferred {
        match solve_arithmetic(&t1, &t2) {
            SolveResult::Solved(bindings) => {
                // 解けた変数を goals に適用
                for (var_name, value) in bindings {
                    let replacement = number(value);
                    substitute_in_goals(goals, &var_name, &replacement);
                }
            }
            SolveResult::Contradiction => {
                return Err(UnifyError {
                    message: format!("arithmetic constraint contradiction: {:?} = {:?}", t1, t2),
                    term1: t1,
                    term2: t2,
                });
            }
            SolveResult::Unsolvable => {
                // 評価できなければエラー
                if eval_arith(&t1).is_none() && matches!(&t1, Term::ArithExpr { .. }) {
                    return Err(UnifyError {
                        message: format!("cannot evaluate arithmetic expression: {:?}", t1),
                        term1: t1,
                        term2: t2,
                    });
                }
                if eval_arith(&t2).is_none() && matches!(&t2, Term::ArithExpr { .. }) {
                    return Err(UnifyError {
                        message: format!("cannot evaluate arithmetic expression: {:?}", t2),
                        term1: t1,
                        term2: t2,
                    });
                }
                // 両方評価できたら一致チェック
                match (&t1, &t2) {
                    (Term::Number { value: n1 }, Term::Number { value: n2 }) if n1 != n2 => {
                        return Err(UnifyError {
                            message: format!("number mismatch: {} != {}", n1, n2),
                            term1: t1,
                            term2: t2,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // goals 内の算術式を評価
    for goal in goals.iter_mut() {
        eval_arith_in_place(goal);
    }

    Ok(())
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
    match term {
        Term::Var { name } => {
            if name == "_" {
                var(name.clone())
            } else {
                var(format!("{}_{}", name, suffix))
            }
        }
        Term::RangeVar { name, min, max } => {
            if name == "_" {
                range_var(name.clone(), *min, *max)
            } else {
                range_var(format!("{}_{}", name, suffix), *min, *max)
            }
        }
        Term::Struct { functor, args } => {
            let new_args = args.iter().map(|a| rename_term_vars(a, suffix)).collect();
            struc(functor.clone(), new_args)
        }
        Term::List { items, tail } => {
            let new_items = items.iter().map(|i| rename_term_vars(i, suffix)).collect();
            let new_tail = tail.as_ref().map(|t| rename_term_vars(t.as_ref(), suffix));
            list(new_items, new_tail)
        }
        Term::ArithExpr { op, left, right } => arith_expr(
            *op,
            rename_term_vars(left, suffix),
            rename_term_vars(right, suffix),
        ),
        Term::Number { .. } => term.clone(),
    }
}

/// 実行トレースの1ステップ
#[derive(Debug, Clone)]
pub struct TraceStep {
    pub selected_goal: Term,
    pub matched_clause: Clause,
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

    /// all_terms[goal_idx] を clause とマッチし、body を挿入する
    /// unify が成功すると all_terms 全体の変数がインプレースで書き換えられる
    /// 返り値: (マッチした clause, body の長さ)
    fn rewrite_step(
        &mut self,
        all_terms: &mut Vec<Term>,
        goal_idx: usize,
    ) -> Result<(Clause, usize), RewriteError> {
        let goal = all_terms[goal_idx].clone();

        for clause in &self.db {
            self.clause_counter += 1;
            let renamed = rename_clause_vars(clause, &self.clause_counter.to_string());

            let (head, body) = match &renamed {
                Clause::Fact(term) => (term.clone(), vec![]),
                Clause::Rule { head, body } => (head.clone(), body.clone()),
            };

            let body_len = body.len();

            // body を goal_idx+1 の位置に挿入した試行用 Vec を作成
            let mut trial = all_terms.clone();
            for (i, b) in body.into_iter().enumerate() {
                trial.insert(goal_idx + 1 + i, b);
            }

            if unify(goal.clone(), head, &mut trial).is_ok() {
                *all_terms = trial;
                return Ok((renamed, body_len));
            }
        }
        Err(RewriteError {
            message: "no clause matches goal".to_string(),
            goal,
        })
    }

    pub fn execute_with_trace(
        &mut self,
        query: Vec<Term>,
    ) -> Result<(Vec<Term>, Vec<TraceStep>), RewriteError> {
        // all_terms: [resolved... | remaining_goals...]
        // resolved_count で境界を管理
        let mut all_terms = query;
        let mut resolved_count = 0;
        let mut trace = Vec::new();

        while resolved_count < all_terms.len() {
            let goal_before = all_terms[resolved_count].clone();
            let (matched_clause, body_len) = self.rewrite_step(&mut all_terms, resolved_count)?;

            let new_goals: Vec<Term> = all_terms[resolved_count + 1..resolved_count + 1 + body_len]
                .to_vec();

            trace.push(TraceStep {
                selected_goal: goal_before,
                matched_clause,
                new_goals,
            });

            resolved_count += 1;
        }

        Ok((all_terms, trace))
    }

    pub fn execute(&mut self, query: Vec<Term>) -> Result<Vec<Term>, RewriteError> {
        let (resolved, _) = self.execute_with_trace(query)?;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{database, query};

    fn run_success(db_src: &str, query_src: &str) -> Vec<Term> {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        interp.execute(q).expect("Expected success")
    }

    fn run_success_with_trace(db_src: &str, query_src: &str) -> (Vec<Term>, Vec<TraceStep>) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        interp.execute_with_trace(q).expect("Expected success")
    }

    fn run_failure(db_src: &str, query_src: &str) {
        let db = database(db_src).expect("failed to parse db");
        let q = query(query_src).expect("failed to parse query").1;
        let mut interp = Interpreter::new(db);
        assert!(interp.execute(q).is_err(), "Expected failure, got success");
    }

    /// resolved_goalsを文字列のVecに変換
    fn resolved_strs(resolved: &[Term]) -> Vec<String> {
        resolved.iter().map(|t| format!("{:?}", t)).collect()
    }

    // ===== unify tests =====

    #[test]
    fn test_unify_vars() {
        let x = var("X".to_string());
        let a = struc("a".to_string(), vec![]);
        let mut goals = vec![var("X".to_string())];
        unify(x, a.clone(), &mut goals).unwrap();
        assert_eq!(goals[0], a);
    }

    #[test]
    fn test_unify_structs() {
        let t1 = struc("f".to_string(), vec![var("X".to_string())]);
        let t2 = struc("f".to_string(), vec![struc("a".to_string(), vec![])]);
        let mut goals = vec![var("X".to_string())];
        unify(t1, t2, &mut goals).unwrap();
        assert_eq!(goals[0], struc("a".to_string(), vec![]));
    }

    #[test]
    fn test_unify_fail() {
        let t1 = struc("f".to_string(), vec![]);
        let t2 = struc("g".to_string(), vec![]);
        assert!(unify(t1, t2, &mut vec![]).is_err());
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
        let mut goals = vec![var("X".to_string())];
        unify(rv, n.clone(), &mut goals).unwrap();
        assert_eq!(goals[0], n);
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
        assert!(unify(rv, n, &mut vec![]).is_err());
    }

    #[test]
    fn test_rangevar_number_boundary_exclusive() {
        use crate::parse::{Bound, number, range_var};
        let make_rv = || {
            range_var(
                "X".to_string(),
                Some(Bound {
                    value: 0,
                    inclusive: false,
                }),
                Some(Bound {
                    value: 10,
                    inclusive: false,
                }),
            )
        };
        assert!(unify(make_rv(), number(0), &mut vec![]).is_err());
        assert!(unify(make_rv(), number(10), &mut vec![]).is_err());
        assert!(unify(make_rv(), number(1), &mut vec![]).is_ok());
        assert!(unify(make_rv(), number(9), &mut vec![]).is_ok());
    }

    #[test]
    fn test_rangevar_number_boundary_inclusive() {
        use crate::parse::{Bound, number, range_var};
        let make_rv = || {
            range_var(
                "X".to_string(),
                Some(Bound {
                    value: 0,
                    inclusive: true,
                }),
                Some(Bound {
                    value: 10,
                    inclusive: true,
                }),
            )
        };
        assert!(unify(make_rv(), number(0), &mut vec![]).is_ok());
        assert!(unify(make_rv(), number(10), &mut vec![]).is_ok());
    }

    #[test]
    fn test_rangevar_intersection() {
        use crate::parse::{Bound, range_var};
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
        let mut goals = vec![var("X".to_string())];
        unify(rv1, rv2, &mut goals).unwrap();
        match &goals[0] {
            Term::RangeVar { min, max, .. } => {
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
        assert!(unify(rv1, rv2, &mut vec![]).is_err());
    }

    #[test]
    fn test_rangevar_intersection_inclusive_exclusive() {
        use crate::parse::{Bound, range_var};
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
        assert!(unify(rv1, rv2, &mut vec![]).is_err());
    }

    // ===== arithmetic tests =====

    #[test]
    fn test_arith_simple_add() {
        use crate::parse::number;
        let expr = crate::parse::arith_expr(crate::parse::ArithOp::Add, number(3), number(5));
        let n = number(8);
        assert!(unify(expr, n, &mut vec![]).is_ok());
    }

    #[test]
    fn test_arith_simple_sub() {
        use crate::parse::number;
        let expr = crate::parse::arith_expr(crate::parse::ArithOp::Sub, number(10), number(3));
        let n = number(7);
        assert!(unify(expr, n, &mut vec![]).is_ok());
    }

    #[test]
    fn test_arith_simple_mul() {
        use crate::parse::number;
        let expr = crate::parse::arith_expr(crate::parse::ArithOp::Mul, number(4), number(5));
        let n = number(20);
        assert!(unify(expr, n, &mut vec![]).is_ok());
    }

    #[test]
    fn test_arith_simple_div() {
        use crate::parse::number;
        let expr = crate::parse::arith_expr(crate::parse::ArithOp::Div, number(10), number(3));
        let n = number(3); // truncated
        assert!(unify(expr, n, &mut vec![]).is_ok());
    }

    #[test]
    fn test_arith_in_rule() {
        let resolved = run_success("cube(3, 7, 3).", "cube(3, 10-3, 3).");
        assert_eq!(resolved_strs(&resolved), vec!["cube(3, 7, 3)"]);
    }

    #[test]
    fn test_arith_with_var() {
        let resolved = run_success("f(5, 5).", "f(X, 10 - X).");
        assert_eq!(resolved_strs(&resolved), vec!["f(5, 5)"]);
    }

    #[test]
    fn test_arith_expr_before_var() {
        let resolved = run_success("f(10, 5).", "f(X * 2, X).");
        assert_eq!(resolved_strs(&resolved), vec!["f(10, 5)"]);
    }

    #[test]
    fn test_arith_multiple_vars_order() {
        let resolved = run_success("f(3, 1, 2).", "f(X + Y, X, Y).");
        assert_eq!(resolved_strs(&resolved), vec!["f(3, 1, 2)"]);
    }

    #[test]
    fn test_arith_precedence() {
        let resolved = run_success("result(14).", "result(2 + 3 * 4).");
        assert_eq!(resolved_strs(&resolved), vec!["result(14)"]);
    }

    #[test]
    fn test_arith_compound_expr() {
        let resolved = run_success("f(10, 2, 3).", "f((X + Y) * 2, X, Y).");
        assert_eq!(resolved_strs(&resolved), vec!["f(10, 2, 3)"]);
    }

    #[test]
    fn test_arith_nested_expr() {
        let resolved = run_success("f(25, 3, 4).", "f(X * X + Y * Y, X, Y).");
        assert_eq!(resolved_strs(&resolved), vec!["f(25, 3, 4)"]);
    }

    #[test]
    fn test_arith_both_sides_expr() {
        let resolved = run_success("f(5).", "f(X).");
        assert_eq!(resolved_strs(&resolved), vec!["f(5)"]);
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
        let resolved = run_success("honi(fuwa).", "honi(X).");
        assert_eq!(resolved_strs(&resolved), vec!["honi(fuwa)"]);
    }

    #[test]
    fn var_to_var_binding() {
        // DBの変数とクエリの変数がマッチ -> resolved goalでは変数のまま
        let resolved = run_success("honi(X).", "honi(Y).");
        // Y_1のような変数名になる
        assert!(resolved_strs(&resolved)[0].starts_with("honi("));
    }

    #[test]
    fn multiple_usages_of_same_variable() {
        let resolved = run_success("likes(X, X).", "likes(fuwa, Y).");
        assert_eq!(resolved_strs(&resolved), vec!["likes(fuwa, fuwa)"]);
    }

    // ===== nested struct tests =====

    #[test]
    fn deep_struct_on_db() {
        let resolved = run_success("a(b(c)).", "a(X).");
        assert_eq!(resolved_strs(&resolved), vec!["a(b(c))"]);
    }

    #[test]
    fn deep_struct_on_query() {
        let resolved = run_success("a(X).", "a(b(c)).");
        assert_eq!(resolved_strs(&resolved), vec!["a(b(c))"]);
    }

    #[test]
    fn recursive_unify_nested_struct_match() {
        let resolved = run_success("f(a(b)).", "f(a(b)).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b))"]);
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
        let resolved = run_success("f(a(X)).", "f(a(b)).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b))"]);
    }

    #[test]
    fn recursive_unify_query_var_binds_in_nested() {
        let resolved = run_success("f(a(b)).", "f(a(X)).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b))"]);
    }

    #[test]
    fn recursive_unify_multiple_args() {
        let resolved = run_success("f(a(b), c(d)).", "f(a(b), c(d)).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b), c(d))"]);
    }

    #[test]
    fn recursive_unify_multiple_args_one_mismatch() {
        run_failure("f(a(b), c(d)).", "f(a(b), c(e)).");
    }

    #[test]
    fn recursive_unify_three_levels_deep() {
        let resolved = run_success("f(a(b(c))).", "f(a(b(c))).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b(c)))"]);
    }

    #[test]
    fn recursive_unify_three_levels_deep_mismatch() {
        run_failure("f(a(b(c))).", "f(a(b(d))).");
    }

    #[test]
    fn recursive_unify_var_at_deep_level() {
        let resolved = run_success("f(a(b(X))).", "f(a(b(c))).");
        assert_eq!(resolved_strs(&resolved), vec!["f(a(b(c)))"]);
    }

    // ===== rule tests =====

    #[test]
    fn resolved_goals_returned() {
        let resolved = run_success("p :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p.");
        assert_eq!(resolved_strs(&resolved), vec!["p", "q(a, b)", "r(b, c)"]);
    }

    #[test]
    fn sample_rule() {
        let resolved = run_success("p(X,Y) :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p(A, B).");
        assert_eq!(
            resolved_strs(&resolved),
            vec!["p(a, c)", "q(a, b)", "r(b, c)"]
        );
    }

    #[test]
    fn rule_single_goal() {
        let resolved = run_success("parent(X) :- father(X). father(tom).", "parent(tom).");
        assert_eq!(resolved_strs(&resolved), vec!["parent(tom)", "father(tom)"]);
    }

    #[test]
    fn rule_single_goal_with_var_query() {
        let resolved = run_success("parent(X) :- father(X). father(tom).", "parent(Y).");
        assert_eq!(resolved_strs(&resolved), vec!["parent(tom)", "father(tom)"]);
    }

    #[test]
    fn grandparent_rule() {
        let db = r#"
            parent(alice, bob).
            parent(bob, carol).
            grandparent(X, Y) :- parent(X, Z), parent(Z, Y).
        "#;
        let resolved = run_success(db, "grandparent(alice, Who).");
        assert_eq!(
            resolved_strs(&resolved),
            vec![
                "grandparent(alice, carol)",
                "parent(alice, bob)",
                "parent(bob, carol)"
            ]
        );
    }

    // ===== list tests =====

    #[test]
    fn list_empty_match() {
        let resolved = run_success("f([]).", "f([]).");
        assert_eq!(resolved_strs(&resolved), vec!["f([])"]);
    }

    #[test]
    fn list_simple_match() {
        let resolved = run_success("f([a, b, c]).", "f([a, b, c]).");
        assert_eq!(resolved_strs(&resolved), vec!["f([a, b, c])"]);
    }

    #[test]
    fn list_mismatch() {
        run_failure("f([a, b]).", "f([a, c]).");
    }

    #[test]
    fn list_var_binding() {
        let resolved = run_success("f([a, b, c]).", "f(X).");
        assert_eq!(resolved_strs(&resolved), vec!["f([a, b, c])"]);
    }

    #[test]
    fn list_head_tail_pattern() {
        let resolved = run_success("f([a, b, c]).", "f([H|T]).");
        assert_eq!(resolved_strs(&resolved), vec!["f([a | [b, c]])"]);
    }

    #[test]
    fn member_first() {
        let resolved = run_success("member(X, [X|_]).", "member(a, [a, b, c]).");
        assert_eq!(resolved_strs(&resolved), vec!["member(a, [a, b, c])"]);
    }

    // ===== trace tests =====

    #[test]
    fn trace_records_rewrite() {
        let db = "q(a, b). p(X, Y) :- q(X, Y).";
        let (resolved, trace) = run_success_with_trace(db, "p(W, V).");

        assert_eq!(trace.len(), 2);
        assert!(matches!(&trace[0].matched_clause, Clause::Rule { .. }));
        assert!(matches!(&trace[1].matched_clause, Clause::Fact(_)));

        assert_eq!(resolved_strs(&resolved), vec!["p(a, b)", "q(a, b)"]);
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
        let resolved = run_success("a(X) :- b(X). b(X) :- c(X). c(foo).", "a(foo).");
        assert_eq!(resolved_strs(&resolved), vec!["a(foo)", "b(foo)", "c(foo)"]);
    }

    #[test]
    fn rule_chain_three_levels() {
        let resolved = run_success(
            "a(X) :- b(X). b(X) :- c(X). c(X) :- d(X). d(bar).",
            "a(bar).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["a(bar)", "b(bar)", "c(bar)", "d(bar)"]
        );
    }

    #[test]
    fn rule_chain_with_var_binding() {
        let resolved = run_success("a(X) :- b(X). b(X) :- c(X). c(baz).", "a(Y).");
        assert_eq!(resolved_strs(&resolved), vec!["a(baz)", "b(baz)", "c(baz)"]);
    }

    // ===== rule with nested struct tests =====

    #[test]
    fn rule_with_nested_struct_in_fact() {
        let resolved = run_success(
            "outer(X) :- inner(X). inner(pair(a, b)).",
            "outer(pair(a, b)).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["outer(pair(a, b))", "inner(pair(a, b))"]
        );
    }

    #[test]
    fn rule_with_nested_struct_var_binding() {
        let resolved = run_success("outer(X) :- inner(X). inner(pair(a, b)).", "outer(Y).");
        assert_eq!(
            resolved_strs(&resolved),
            vec!["outer(pair(a, b))", "inner(pair(a, b))"]
        );
    }

    #[test]
    fn rule_with_deeply_nested_struct() {
        let resolved = run_success(
            "wrap(X) :- data(X). data(node(leaf(a), leaf(b))).",
            "wrap(node(leaf(a), leaf(b))).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec![
                "wrap(node(leaf(a), leaf(b)))",
                "data(node(leaf(a), leaf(b)))"
            ]
        );
    }

    #[test]
    fn rule_shared_variable_in_body() {
        let resolved = run_success("same(X) :- eq(X, X). eq(a, a).", "same(a).");
        assert_eq!(resolved_strs(&resolved), vec!["same(a)", "eq(a, a)"]);
    }

    // ===== rule with multiple args =====

    #[test]
    fn rule_three_args() {
        let resolved = run_success(
            "triple(X, Y, Z) :- first(X), second(Y), third(Z). first(a). second(b). third(c).",
            "triple(A, B, C).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["triple(a, b, c)", "first(a)", "second(b)", "third(c)"]
        );
    }

    // ===== rule head with struct =====

    #[test]
    fn rule_head_with_struct() {
        let resolved = run_success(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(pair(a, b)).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["make_pair(pair(a, b))", "left(a)", "right(b)"]
        );
    }

    #[test]
    fn rule_head_with_struct_var_query() {
        let resolved = run_success(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(P).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["make_pair(pair(a, b))", "left(a)", "right(b)"]
        );
    }

    // ===== backtracking required (ignored for now) =====

    #[test]
    #[ignore]
    fn rule_multiple_goals() {
        let resolved = run_success(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, c).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["grandparent(a, c)", "parent(a, b)", "parent(b, c)"]
        );
    }

    #[test]
    #[ignore]
    fn rule_multiple_goals_with_var() {
        let resolved = run_success(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, W).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["grandparent(a, c)", "parent(a, b)", "parent(b, c)"]
        );
    }

    #[test]
    #[ignore]
    fn rule_shared_variable_propagation() {
        let resolved = run_success(
            "connect(X, Z) :- link(X, Y), link(Y, Z). link(a, b). link(b, c).",
            "connect(a, Z).",
        );
        assert_eq!(
            resolved_strs(&resolved),
            vec!["connect(a, c)", "link(a, b)", "link(b, c)"]
        );
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
