use std::collections::HashMap;

use crate::parse::{ArithOp, Bound, FixedPoint, Term};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SolverState {
    /// 確定値 (変数名 -> 値)
    exacts: HashMap<String, FixedPoint>,
    /// 未解決の制約
    constraints: Vec<ArithConstraint>,
    /// 矛盾が発生した場合のエラーメッセージ
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArithVar(String);

// とりあえず線形な制約のみ
#[derive(Debug, Clone, PartialEq)]
pub enum ArithConstraint {
    Eq(ArithVar, ArithVar),
    Plus(ArithVar, ArithVar, FixedPoint),  // X = Y + c
    Mul(ArithVar, ArithVar, FixedPoint),   // X = Y * c
    Minus(ArithVar, FixedPoint, ArithVar), // X = c - Y
}

impl SolverState {
    pub fn new(expr_eqs: Vec<ArithEq>) -> Self {
        let constraints = expr_eqs
            .into_iter()
            .flat_map(|eq| Self::extract_constraints(&eq))
            .collect();
        Self {
            constraints,
            ..Self::default()
        }
    }

    fn extract_constraints(eq: &ArithEq) -> Vec<ArithConstraint> {
        match eq {
            ArithEq { left, right } => {
                todo!()
            }
        }
    }

    pub fn put_exact(&mut self, var: String, value: FixedPoint) {
        if let Some(&existing) = self.exacts.get(&var) {
            if existing != value {
                self.error = Some(format!(
                    "contradiction: {} already has value {}, cannot assign {}",
                    var, existing, value
                ));
            }
        } else {
            self.exacts.insert(var, value);
        }
    }

    pub fn get_value(&self, var: &str) -> Option<FixedPoint> {
        if let Some(&v) = self.exacts.get(var) {
            return Some(v);
        }
        None
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn get_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn exacts(&self) -> &HashMap<String, FixedPoint> {
        &self.exacts
    }

    pub fn remaining_constraints(&self) -> &[ArithConstraint] {
        &self.constraints
    }

    fn solve_step(&mut self) -> bool {
        let made_progress = false;
        let constraints = std::mem::take(&mut self.constraints);
        let remaining = Vec::new();

        for _constraint in constraints {
            todo!()
        }
        self.constraints = remaining;
        made_progress
    }

    pub fn repeat_until_fixpoint(&mut self) {
        while self.solve_step() {}
    }
}

/// Termの算術式サブセット
#[derive(Debug, Clone, PartialEq)]
pub enum ArithExpr {
    /// 変数
    Var(String),
    /// 範囲制約付き変数
    RangeVar {
        name: String,
        min: Option<Bound>,
        max: Option<Bound>,
    },
    /// 数値リテラル
    Num(FixedPoint),
    /// 二項演算
    BinOp {
        op: ArithOp,
        left: Box<ArithExpr>,
        right: Box<ArithExpr>,
    },
}

impl ArithExpr {
    pub fn var(name: impl Into<String>) -> Self {
        ArithExpr::Var(name.into())
    }

    pub fn num(value: FixedPoint) -> Self {
        ArithExpr::Num(value)
    }

    pub fn num_int(value: i64) -> Self {
        ArithExpr::Num(FixedPoint::from_int(value))
    }
}

impl std::ops::Add for ArithExpr {
    type Output = ArithExpr;
    fn add(self, rhs: Self) -> Self::Output {
        ArithExpr::BinOp {
            op: ArithOp::Add,
            left: Box::new(self),
            right: Box::new(rhs),
        }
    }
}

impl std::ops::Add<i64> for ArithExpr {
    type Output = ArithExpr;
    fn add(self, rhs: i64) -> Self::Output {
        self + ArithExpr::Num(FixedPoint::from_int(rhs))
    }
}

impl std::ops::Sub for ArithExpr {
    type Output = ArithExpr;
    fn sub(self, rhs: Self) -> Self::Output {
        ArithExpr::BinOp {
            op: ArithOp::Sub,
            left: Box::new(self),
            right: Box::new(rhs),
        }
    }
}

impl std::ops::Sub<i64> for ArithExpr {
    type Output = ArithExpr;
    fn sub(self, rhs: i64) -> Self::Output {
        self - ArithExpr::Num(FixedPoint::from_int(rhs))
    }
}

impl std::ops::Mul for ArithExpr {
    type Output = ArithExpr;
    fn mul(self, rhs: Self) -> Self::Output {
        ArithExpr::BinOp {
            op: ArithOp::Mul,
            left: Box::new(self),
            right: Box::new(rhs),
        }
    }
}

impl std::ops::Mul<i64> for ArithExpr {
    type Output = ArithExpr;
    fn mul(self, rhs: i64) -> Self::Output {
        self * ArithExpr::Num(FixedPoint::from_int(rhs))
    }
}

impl std::ops::Div for ArithExpr {
    type Output = ArithExpr;
    fn div(self, rhs: Self) -> Self::Output {
        ArithExpr::BinOp {
            op: ArithOp::Div,
            left: Box::new(self),
            right: Box::new(rhs),
        }
    }
}

impl std::ops::Div<i64> for ArithExpr {
    type Output = ArithExpr;
    fn div(self, rhs: i64) -> Self::Output {
        self / ArithExpr::Num(FixedPoint::from_int(rhs))
    }
}

impl From<i64> for ArithExpr {
    fn from(value: i64) -> Self {
        ArithExpr::Num(FixedPoint::from_int(value))
    }
}

/// 算術制約: left = right
#[derive(Debug, Clone, PartialEq)]
pub struct ArithEq {
    pub left: ArithExpr,
    pub right: ArithExpr,
}

impl ArithEq {
    pub fn new(left: ArithExpr, right: ArithExpr) -> Self {
        Self { left, right }
    }

    pub fn eq(left: impl Into<ArithExpr>, right: impl Into<ArithExpr>) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

/// Term から ArithExpr への変換エラー
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionError {
    pub message: String,
}

impl ArithExpr {
    /// Term から ArithExpr への変換を試みる
    /// Struct や List など算術式でないものは Err を返す
    pub fn try_from_term(term: &Term) -> Result<Self, ConversionError> {
        match term {
            Term::Var { name } => Ok(ArithExpr::Var(name.clone())),
            Term::DefaultVar { value, .. } => Ok(ArithExpr::Num(*value)),
            Term::RangeVar { name, min, max } => Ok(ArithExpr::RangeVar {
                name: name.clone(),
                min: *min,
                max: *max,
            }),
            Term::Number { value } => Ok(ArithExpr::Num(*value)),
            Term::InfixExpr { op, left, right } => {
                let left = ArithExpr::try_from_term(left)?;
                let right = ArithExpr::try_from_term(right)?;
                Ok(ArithExpr::BinOp {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            Term::Struct { functor, .. } => Err(ConversionError {
                message: format!(
                    "cannot convert struct '{}' to arithmetic expression",
                    functor
                ),
            }),
            Term::List { .. } => Err(ConversionError {
                message: "cannot convert list to arithmetic expression".to_string(),
            }),
        }
    }

    /// ArithExpr を Term に変換
    pub fn to_term(&self) -> Term {
        use crate::parse::{arith_expr, number, range_var, var};
        match self {
            ArithExpr::Var(name) => var(name.clone()),
            ArithExpr::RangeVar { name, min, max } => range_var(name.clone(), *min, *max),
            ArithExpr::Num(value) => number(*value),
            ArithExpr::BinOp { op, left, right } => {
                arith_expr(*op, left.to_term(), right.to_term())
            }
        }
    }

    /// 式中の未束縛変数を収集
    pub fn collect_vars(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_vars_rec(&mut vars);
        vars
    }

    fn collect_vars_rec(&self, vars: &mut Vec<String>) {
        match self {
            ArithExpr::Var(name) if name != "_" => {
                if !vars.contains(name) {
                    vars.push(name.clone());
                }
            }
            ArithExpr::RangeVar { name, .. } => {
                if !vars.contains(name) {
                    vars.push(name.clone());
                }
            }
            ArithExpr::BinOp { left, right, .. } => {
                left.collect_vars_rec(vars);
                right.collect_vars_rec(vars);
            }
            _ => {}
        }
    }
}

/// 算術式用の代入（変数名 -> 整数値）
pub type ArithSubstitution = HashMap<String, FixedPoint>;

// ============================================================
// ソルバーの結果
// ============================================================

/// 制約ソルバーの結果
#[derive(Debug, Clone, PartialEq)]
pub enum SolveResult {
    /// 解が見つかった（変数名 -> 値 の代入を返す）
    Solved(HashMap<String, FixedPoint>),
    /// 制約が矛盾している（例: 5 = 6）
    Contradiction,
    /// 解けない（未束縛変数が複数ある等）- 次のソルバーに委譲
    Unsolvable,
}

// ============================================================
// メインの solve 関数
// ============================================================

pub fn solve_arithmetic(_left: &Term, _right: &Term) -> SolveResult {
    // TODO: 算術制約ソルバーの実装
    SolveResult::Unsolvable
}

// ============================================================
// テスト
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ArithOp, FixedPoint, arith_expr, number_int, var};

    fn x() -> ArithExpr {
        ArithExpr::var("X")
    }
    fn y() -> ArithExpr {
        ArithExpr::var("Y")
    }

    fn v(name: &str) -> Term {
        var(name.to_string())
    }

    /// SolverStateで制約を解いて結果を検証
    fn solve(constraint: ArithEq) -> SolverState {
        let mut state = SolverState::new(vec![constraint]);
        state.repeat_until_fixpoint();
        state
    }

    // ===== ArithExpr operator tests =====

    #[test]
    fn test_arith_expr_operators() {
        // X + 1
        assert_eq!(
            x() + 1,
            ArithExpr::BinOp {
                op: ArithOp::Add,
                left: Box::new(ArithExpr::Var("X".to_string())),
                right: Box::new(ArithExpr::Num(FixedPoint::from_int(1))),
            }
        );

        // X + Y の変数収集
        let vars = (x() + y()).collect_vars();
        assert_eq!(vars, vec!["X".to_string(), "Y".to_string()]);
    }

    #[test]
    fn test_arith_expr_from_term() {
        let term = arith_expr(ArithOp::Add, v("X"), number_int(1));
        let expr = ArithExpr::try_from_term(&term).unwrap();
        assert_eq!(expr, x() + 1);
    }

    // ===== LinearSolver tests =====

    #[test]
    fn test_linear_simple_addition() {
        // X + 1 = 6 -> X = 5
        let state = solve(ArithEq::eq(x() + 1, 6));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(5)));
    }

    #[test]
    fn test_linear_simple_subtraction() {
        // X - 3 = 7 -> X = 10
        let state = solve(ArithEq::eq(x() - 3, 7));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(10)));
    }

    #[test]
    fn test_linear_variable_on_right() {
        // 6 = X + 1 -> X = 5
        let state = solve(ArithEq::eq(6, x() + 1));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(5)));
    }

    #[test]
    fn test_linear_multiplication() {
        // X * 2 = 10 -> X = 5
        let state = solve(ArithEq::eq(x() * 2, 10));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(5)));
    }

    #[test]
    fn test_linear_complex_expression() {
        // 2 * X + 3 = 11 -> X = 4
        let state = solve(ArithEq::eq(ArithExpr::num_int(2) * x() + 3, 11));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(4)));
    }

    #[test]
    fn test_linear_nested_expression() {
        // (X + 1) * 3 = 12 -> X = 3
        let state = solve(ArithEq::eq((x() + 1) * 3, 12));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(3)));
    }

    #[test]
    fn test_linear_negative_result() {
        // X + 10 = 3 -> X = -7
        let state = solve(ArithEq::eq(x() + 10, 3));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(-7)));
    }

    // ===== DivisionSolver tests =====

    #[test]
    fn test_division_simple() {
        // X / 2 = 5 -> X = 10
        let state = solve(ArithEq::eq(x() / 2, 5));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(10)));
    }

    #[test]
    fn test_division_with_offset() {
        // (X + 1) / 3 = 4 -> X = 11
        let state = solve(ArithEq::eq((x() + 1) / 3, 4));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(11)));
    }

    #[test]
    fn test_division_negative_divisor() {
        // X / -2 = 5 -> X = -10
        let state = solve(ArithEq::eq(x() / ArithExpr::num_int(-2), 5));
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(-10)));
    }

    // ===== General tests =====

    #[test]
    fn test_contradiction() {
        // 5 = 6 -> error
        let state = solve(ArithEq::eq(5, 6));
        assert!(state.has_error());
    }

    #[test]
    fn test_two_variables_unsolvable() {
        // X + Y = 10 -> unsolvable (残る)
        let state = solve(ArithEq::eq(x() + y(), 10));
        assert!(!state.has_error());
        assert!(!state.remaining_constraints().is_empty());
    }

    #[test]
    fn test_no_integer_solution() {
        // X * 2 = 5 -> unsolvable (残る)
        let state = solve(ArithEq::eq(x() * 2, 5));
        assert!(!state.has_error());
        assert!(!state.remaining_constraints().is_empty());
    }

    #[test]
    fn test_both_sides_constant_equal() {
        // 5 = 5 -> ok
        let state = solve(ArithEq::eq(5, 5));
        assert!(!state.has_error());
        assert!(state.remaining_constraints().is_empty());
    }

    #[test]
    fn test_division_by_variable_unsolvable() {
        // 6 / X = 2 -> unsolvable (残る)
        let state = solve(ArithEq::eq(ArithExpr::num_int(6) / x(), 2));
        assert!(!state.has_error());
        assert!(!state.remaining_constraints().is_empty());
    }

    #[test]
    fn test_solver_state_from_constraints() {
        // X + 1 = 6
        let constraints = vec![ArithEq::eq(x() + 1, 6)];

        let mut state = SolverState::new(constraints);
        state.repeat_until_fixpoint();

        assert!(!state.has_error());
        assert_eq!(state.get_value("X"), Some(FixedPoint::from_int(5)));
    }
}
