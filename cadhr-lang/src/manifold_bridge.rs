//! Prolog Term -> manifold-rs Manifold 変換層
//!
//! Term（書き換え後の項）を ManifoldExpr 中間表現に変換し、
//! それを manifold-rs の Manifold オブジェクトに評価する。

use crate::parse::{ArithOp, Term};
use manifold_rs::{Manifold, Mesh};
use std::fmt;
use std::str::FromStr;
use strum_macros::{EnumIter, EnumString, IntoStaticStr};

const DEFAULT_SEGMENTS: u32 = 32;

/// ビルトインプリミティブのfunctor名を表すenum
/// strumによりfunctor文字列との相互変換が可能
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, IntoStaticStr, EnumIter)]
#[strum(serialize_all = "lowercase")]
pub enum BuiltinFunctor {
    // プリミティブ
    Cube,
    Sphere,
    Cylinder,
    Tetrahedron,
    // CSG演算
    Union,
    Difference,
    Intersection,
    // 変形
    Translate,
    Scale,
    Rotate,
}

/// functor名がビルトインプリミティブかどうかを判定
pub fn is_builtin_functor(functor: &str) -> bool {
    BuiltinFunctor::from_str(functor).is_ok()
}

/// 変換エラー
#[derive(Debug, Clone)]
pub enum ConversionError {
    /// 未知のプリミティブ/functor
    UnknownPrimitive(String),
    /// 引数の数が不一致
    ArityMismatch {
        functor: String,
        expected: String,
        got: usize,
    },
    /// 引数の型が不正
    TypeMismatch {
        functor: String,
        arg_index: usize,
        expected: &'static str,
    },
    /// 未束縛変数
    UnboundVariable(String),
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::UnknownPrimitive(name) => {
                write!(f, "Unknown primitive: {}", name)
            }
            ConversionError::ArityMismatch {
                functor,
                expected,
                got,
            } => {
                write!(
                    f,
                    "Arity mismatch for {}: expected {}, got {}",
                    functor, expected, got
                )
            }
            ConversionError::TypeMismatch {
                functor,
                arg_index,
                expected,
            } => {
                write!(
                    f,
                    "Type mismatch for {} arg {}: expected {}",
                    functor, arg_index, expected
                )
            }
            ConversionError::UnboundVariable(name) => {
                write!(f, "Unbound variable: {}", name)
            }
        }
    }
}

impl std::error::Error for ConversionError {}

/// 引数抽出用ヘルパー
struct Args<'a> {
    args: &'a [Term],
    functor: &'a str,
}

impl<'a> Args<'a> {
    fn new(functor: &'a str, args: &'a [Term]) -> Self {
        Self { args, functor }
    }

    fn len(&self) -> usize {
        self.args.len()
    }

    fn f64(&self, i: usize) -> Result<f64, ConversionError> {
        match &self.args[i] {
            Term::Number { value } => Ok(*value as f64),
            Term::DefaultVar { value, .. } => Ok(*value as f64),
            Term::Var { name } | Term::RangeVar { name, .. } => {
                Err(ConversionError::UnboundVariable(name.clone()))
            }
            _ => Err(ConversionError::TypeMismatch {
                functor: self.functor.to_string(),
                arg_index: i,
                expected: "number",
            }),
        }
    }

    fn u32(&self, i: usize) -> Result<u32, ConversionError> {
        match &self.args[i] {
            Term::Number { value } if *value >= 0 => Ok(*value as u32),
            Term::Number { .. } => Err(ConversionError::TypeMismatch {
                functor: self.functor.to_string(),
                arg_index: i,
                expected: "non-negative integer",
            }),
            Term::DefaultVar { value, .. } if *value >= 0 => Ok(*value as u32),
            Term::DefaultVar { .. } => Err(ConversionError::TypeMismatch {
                functor: self.functor.to_string(),
                arg_index: i,
                expected: "non-negative integer",
            }),
            Term::Var { name } | Term::RangeVar { name, .. } => {
                Err(ConversionError::UnboundVariable(name.clone()))
            }
            _ => Err(ConversionError::TypeMismatch {
                functor: self.functor.to_string(),
                arg_index: i,
                expected: "integer",
            }),
        }
    }

    fn term(&self, i: usize) -> Result<ManifoldExpr, ConversionError> {
        ManifoldExpr::from_term(&self.args[i])
    }

    fn arity_error(&self, expected: &str) -> ConversionError {
        ConversionError::ArityMismatch {
            functor: self.functor.to_string(),
            expected: expected.to_string(),
            got: self.len(),
        }
    }
}

/// manifold-rs APIへの中間表現
#[derive(Debug, Clone)]
pub enum ManifoldExpr {
    // プリミティブ
    Cube {
        x: f64,
        y: f64,
        z: f64,
    },
    Sphere {
        radius: f64,
        segments: u32,
    },
    Cylinder {
        radius: f64,
        height: f64,
        segments: u32,
    },
    Tetrahedron,

    // CSG演算
    Union(Box<ManifoldExpr>, Box<ManifoldExpr>),
    Difference(Box<ManifoldExpr>, Box<ManifoldExpr>),
    Intersection(Box<ManifoldExpr>, Box<ManifoldExpr>),

    // 変形
    Translate {
        expr: Box<ManifoldExpr>,
        x: f64,
        y: f64,
        z: f64,
    },
    Scale {
        expr: Box<ManifoldExpr>,
        x: f64,
        y: f64,
        z: f64,
    },
    Rotate {
        expr: Box<ManifoldExpr>,
        x: f64,
        y: f64,
        z: f64,
    },
}

impl ManifoldExpr {
    /// Prolog Term から ManifoldExpr へ変換
    pub fn from_term(term: &Term) -> Result<Self, ConversionError> {
        match term {
            Term::Struct { functor, args } => Self::from_struct(functor, args),
            Term::InfixExpr { op, left, right } => Self::from_infix_expr(*op, left, right),
            Term::Var { name } => Err(ConversionError::UnboundVariable(name.clone())),
            Term::RangeVar { name, .. } => Err(ConversionError::UnboundVariable(name.clone())),
            _ => Err(ConversionError::UnknownPrimitive(format!("{:?}", term))),
        }
    }

    /// 中置演算子をCAD操作として変換
    /// + -> union, - -> difference, * -> intersection
    fn from_infix_expr(op: ArithOp, left: &Term, right: &Term) -> Result<Self, ConversionError> {
        let left_expr = Box::new(Self::from_term(left)?);
        let right_expr = Box::new(Self::from_term(right)?);

        match op {
            ArithOp::Add => Ok(ManifoldExpr::Union(left_expr, right_expr)),
            ArithOp::Sub => Ok(ManifoldExpr::Difference(left_expr, right_expr)),
            ArithOp::Mul => Ok(ManifoldExpr::Intersection(left_expr, right_expr)),
            ArithOp::Div => Err(ConversionError::UnknownPrimitive(
                "division operator (/) is not supported for CAD operations".to_string(),
            )),
        }
    }

    fn from_struct(functor: &str, args: &[Term]) -> Result<Self, ConversionError> {
        let a = Args::new(functor, args);

        let builtin = BuiltinFunctor::from_str(functor)
            .map_err(|_| ConversionError::UnknownPrimitive(functor.to_string()))?;

        match builtin {
            // プリミティブ
            BuiltinFunctor::Cube if a.len() == 3 => {
                let (x, y, z) = (a.f64(0)?, a.f64(1)?, a.f64(2)?);
                Ok(ManifoldExpr::Cube { x, y, z })
            }
            BuiltinFunctor::Cube => Err(a.arity_error("3")),

            BuiltinFunctor::Sphere if a.len() == 1 => Ok(ManifoldExpr::Sphere {
                radius: a.f64(0)?,
                segments: DEFAULT_SEGMENTS,
            }),
            BuiltinFunctor::Sphere if a.len() == 2 => Ok(ManifoldExpr::Sphere {
                radius: a.f64(0)?,
                segments: a.u32(1)?,
            }),
            BuiltinFunctor::Sphere => Err(a.arity_error("1 or 2")),

            BuiltinFunctor::Cylinder if a.len() == 2 => Ok(ManifoldExpr::Cylinder {
                radius: a.f64(0)?,
                height: a.f64(1)?,
                segments: DEFAULT_SEGMENTS,
            }),
            BuiltinFunctor::Cylinder if a.len() == 3 => Ok(ManifoldExpr::Cylinder {
                radius: a.f64(0)?,
                height: a.f64(1)?,
                segments: a.u32(2)?,
            }),
            BuiltinFunctor::Cylinder => Err(a.arity_error("2 or 3")),

            BuiltinFunctor::Tetrahedron if a.len() == 0 => Ok(ManifoldExpr::Tetrahedron),
            BuiltinFunctor::Tetrahedron => Err(a.arity_error("0")),

            // CSG演算
            BuiltinFunctor::Union if a.len() == 2 => Ok(ManifoldExpr::Union(
                Box::new(a.term(0)?),
                Box::new(a.term(1)?),
            )),
            BuiltinFunctor::Union => Err(a.arity_error("2")),

            BuiltinFunctor::Difference if a.len() == 2 => Ok(ManifoldExpr::Difference(
                Box::new(a.term(0)?),
                Box::new(a.term(1)?),
            )),
            BuiltinFunctor::Difference => Err(a.arity_error("2")),

            BuiltinFunctor::Intersection if a.len() == 2 => Ok(ManifoldExpr::Intersection(
                Box::new(a.term(0)?),
                Box::new(a.term(1)?),
            )),
            BuiltinFunctor::Intersection => Err(a.arity_error("2")),

            // 変形
            BuiltinFunctor::Translate if a.len() == 4 => Ok(ManifoldExpr::Translate {
                expr: Box::new(a.term(0)?),
                x: a.f64(1)?,
                y: a.f64(2)?,
                z: a.f64(3)?,
            }),
            BuiltinFunctor::Translate => Err(a.arity_error("4")),

            BuiltinFunctor::Scale if a.len() == 4 => Ok(ManifoldExpr::Scale {
                expr: Box::new(a.term(0)?),
                x: a.f64(1)?,
                y: a.f64(2)?,
                z: a.f64(3)?,
            }),
            BuiltinFunctor::Scale => Err(a.arity_error("4")),

            BuiltinFunctor::Rotate if a.len() == 4 => Ok(ManifoldExpr::Rotate {
                expr: Box::new(a.term(0)?),
                x: a.f64(1)?,
                y: a.f64(2)?,
                z: a.f64(3)?,
            }),
            BuiltinFunctor::Rotate => Err(a.arity_error("4")),
        }
    }

    /// ManifoldExpr を manifold-rs の Manifold に評価
    pub fn evaluate(&self) -> Manifold {
        match self {
            // プリミティブ
            ManifoldExpr::Cube { x, y, z } => Manifold::cube(*x, *y, *z),
            ManifoldExpr::Sphere { radius, segments } => Manifold::sphere(*radius, *segments),
            ManifoldExpr::Cylinder {
                radius,
                height,
                segments,
            } => Manifold::cylinder(*radius, *radius, *height, *segments),
            ManifoldExpr::Tetrahedron => Manifold::tetrahedron(),

            // CSG
            ManifoldExpr::Union(a, b) => a.evaluate().union(&b.evaluate()),
            ManifoldExpr::Difference(a, b) => a.evaluate().difference(&b.evaluate()),
            ManifoldExpr::Intersection(a, b) => a.evaluate().intersection(&b.evaluate()),

            // 変形
            ManifoldExpr::Translate { expr, x, y, z } => expr.evaluate().translate(*x, *y, *z),
            ManifoldExpr::Scale { expr, x, y, z } => expr.evaluate().scale(*x, *y, *z),
            ManifoldExpr::Rotate { expr, x, y, z } => expr.evaluate().rotate(*x, *y, *z),
        }
    }

    /// ManifoldExpr を Mesh に変換（法線計算込み）
    pub fn to_mesh(&self) -> Mesh {
        let manifold = self.evaluate();
        let with_normals = manifold.calculate_normals(0, 30.0);
        with_normals.to_mesh()
    }
}

/// 複数のTermからMeshを生成する（全てをunionする）
pub fn generate_mesh_from_terms(terms: &[Term]) -> Result<Mesh, ConversionError> {
    if terms.is_empty() {
        return Err(ConversionError::UnknownPrimitive(
            "empty term list".to_string(),
        ));
    }

    let exprs: Vec<ManifoldExpr> = terms
        .iter()
        .map(ManifoldExpr::from_term)
        .collect::<Result<Vec<_>, _>>()?;

    // 全てのManifoldExprをunionで結合
    let manifold = exprs
        .into_iter()
        .map(|e| e.evaluate())
        .reduce(|acc, m| acc.union(&m))
        .unwrap(); // exprsが空でないことは上でチェック済み

    let with_normals = manifold.calculate_normals(0, 30.0);
    Ok(with_normals.to_mesh())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{number, struc, var};

    #[test]
    fn test_cube_conversion() {
        let term = struc("cube".into(), vec![number(10), number(20), number(30)]);
        let expr = ManifoldExpr::from_term(&term).unwrap();
        match expr {
            ManifoldExpr::Cube { x, y, z } => {
                assert_eq!(x, 10.0);
                assert_eq!(y, 20.0);
                assert_eq!(z, 30.0);
            }
            _ => panic!("Expected Cube"),
        }
    }

    #[test]
    fn test_sphere_default_segments() {
        let term = struc("sphere".into(), vec![number(5)]);
        let expr = ManifoldExpr::from_term(&term).unwrap();
        match expr {
            ManifoldExpr::Sphere { radius, segments } => {
                assert_eq!(radius, 5.0);
                assert_eq!(segments, DEFAULT_SEGMENTS);
            }
            _ => panic!("Expected Sphere"),
        }
    }

    #[test]
    fn test_sphere_explicit_segments() {
        let term = struc("sphere".into(), vec![number(5), number(16)]);
        let expr = ManifoldExpr::from_term(&term).unwrap();
        match expr {
            ManifoldExpr::Sphere { radius, segments } => {
                assert_eq!(radius, 5.0);
                assert_eq!(segments, 16);
            }
            _ => panic!("Expected Sphere"),
        }
    }

    #[test]
    fn test_cylinder_default_segments() {
        let term = struc("cylinder".into(), vec![number(3), number(10)]);
        let expr = ManifoldExpr::from_term(&term).unwrap();
        match expr {
            ManifoldExpr::Cylinder {
                radius,
                height,
                segments,
            } => {
                assert_eq!(radius, 3.0);
                assert_eq!(height, 10.0);
                assert_eq!(segments, DEFAULT_SEGMENTS);
            }
            _ => panic!("Expected Cylinder"),
        }
    }

    #[test]
    fn test_union_conversion() {
        let cube1 = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let cube2 = struc("cube".into(), vec![number(2), number(2), number(2)]);
        let union_term = struc("union".into(), vec![cube1, cube2]);
        let expr = ManifoldExpr::from_term(&union_term).unwrap();
        assert!(matches!(expr, ManifoldExpr::Union(_, _)));
    }

    #[test]
    fn test_translate_conversion() {
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let translated = struc(
            "translate".into(),
            vec![cube, number(5), number(10), number(15)],
        );
        let expr = ManifoldExpr::from_term(&translated).unwrap();
        match expr {
            ManifoldExpr::Translate { x, y, z, .. } => {
                assert_eq!(x, 5.0);
                assert_eq!(y, 10.0);
                assert_eq!(z, 15.0);
            }
            _ => panic!("Expected Translate"),
        }
    }

    #[test]
    fn test_unbound_variable_error() {
        let term = struc("cube".into(), vec![var("X".into()), number(1), number(1)]);
        let result = ManifoldExpr::from_term(&term);
        assert!(matches!(result, Err(ConversionError::UnboundVariable(_))));
    }

    #[test]
    fn test_arity_mismatch() {
        let term = struc("cube".into(), vec![number(1), number(2)]);
        let result = ManifoldExpr::from_term(&term);
        assert!(matches!(result, Err(ConversionError::ArityMismatch { .. })));
    }

    #[test]
    fn test_unknown_primitive() {
        let term = struc("unknown_shape".into(), vec![number(1)]);
        let result = ManifoldExpr::from_term(&term);
        assert!(matches!(result, Err(ConversionError::UnknownPrimitive(_))));
    }

    #[test]
    fn test_nested_csg() {
        // difference(union(cube(1,1,1), cube(2,2,2)), sphere(1))
        let cube1 = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let cube2 = struc("cube".into(), vec![number(2), number(2), number(2)]);
        let union_term = struc("union".into(), vec![cube1, cube2]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let diff = struc("difference".into(), vec![union_term, sphere]);

        let expr = ManifoldExpr::from_term(&diff).unwrap();
        assert!(matches!(expr, ManifoldExpr::Difference(_, _)));
    }

    #[test]
    fn test_operator_union() {
        use crate::parse::ArithOp;
        use crate::parse::arith_expr;

        // cube(1,1,1) + sphere(1) -> union
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let add_term = arith_expr(ArithOp::Add, cube, sphere);

        let expr = ManifoldExpr::from_term(&add_term).unwrap();
        assert!(matches!(expr, ManifoldExpr::Union(_, _)));
    }

    #[test]
    fn test_operator_difference() {
        use crate::parse::ArithOp;
        use crate::parse::arith_expr;

        // cube(1,1,1) - sphere(1) -> difference
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let sub_term = arith_expr(ArithOp::Sub, cube, sphere);

        let expr = ManifoldExpr::from_term(&sub_term).unwrap();
        assert!(matches!(expr, ManifoldExpr::Difference(_, _)));
    }

    #[test]
    fn test_operator_intersection() {
        use crate::parse::ArithOp;
        use crate::parse::arith_expr;

        // cube(1,1,1) * sphere(1) -> intersection
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let mul_term = arith_expr(ArithOp::Mul, cube, sphere);

        let expr = ManifoldExpr::from_term(&mul_term).unwrap();
        assert!(matches!(expr, ManifoldExpr::Intersection(_, _)));
    }

    #[test]
    fn test_operator_nested() {
        use crate::parse::ArithOp;
        use crate::parse::arith_expr;

        // (cube(1,1,1) + sphere(1)) - cylinder(1,2)
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let cylinder = struc("cylinder".into(), vec![number(1), number(2)]);

        let union_term = arith_expr(ArithOp::Add, cube, sphere);
        let diff_term = arith_expr(ArithOp::Sub, union_term, cylinder);

        let expr = ManifoldExpr::from_term(&diff_term).unwrap();
        match expr {
            ManifoldExpr::Difference(left, _) => {
                assert!(matches!(*left, ManifoldExpr::Union(_, _)));
            }
            _ => panic!("Expected Difference"),
        }
    }

    #[test]
    fn test_operator_division_error() {
        use crate::parse::ArithOp;
        use crate::parse::arith_expr;

        // cube(1,1,1) / sphere(1) -> error
        let cube = struc("cube".into(), vec![number(1), number(1), number(1)]);
        let sphere = struc("sphere".into(), vec![number(1)]);
        let div_term = arith_expr(ArithOp::Div, cube, sphere);

        let result = ManifoldExpr::from_term(&div_term);
        assert!(matches!(result, Err(ConversionError::UnknownPrimitive(_))));
    }
}
