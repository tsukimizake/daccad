//! 幾何プリミティブ。runtime の Model ツリーと sema/sketch の静的評価値が共有する。
//! 言語側の `p2` / `p3` builtin (Point2D / Point3D) の Rust 側対応物。

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 2D 線分 (`line a b`)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg2 {
    pub a: P2,
    pub b: P2,
}

pub fn p2(x: f64, y: f64) -> P2 {
    P2 { x, y }
}

pub fn p3(x: f64, y: f64, z: f64) -> P3 {
    P3 { x, y, z }
}

/// sketch スカラー式でも使える数学 builtin の名前。
pub const MATH_FNS: [&str; 4] = ["sqrt", "sin", "cos", "tan"];

/// sketch スカラー式でも使える数学 builtin (`sqrt` / `sin` / `cos` / `tan`) の
/// 評価と定義域チェック。runtime の builtin 実行・sema/sketch の静的検査・
/// sketch.rs の forward eval が共有する唯一の実装。
/// 該当しない関数名は `None`。三角関数の角度は度。
pub fn eval_math_fn(name: &str, v: f64) -> Option<Result<f64, String>> {
    Some(match name {
        "sqrt" if v < 0.0 => Err(format!("sqrt: 負の数 ({v}) の平方根は計算できません")),
        "sqrt" => Ok(v.sqrt()),
        "sin" => Ok(v.to_radians().sin()),
        "cos" => Ok(v.to_radians().cos()),
        // 度なら未定義点 (90 + 180k) が f64 で正確に表せるので等値判定できる
        "tan" if (v % 180.0).abs() == 90.0 => {
            Err(format!("tan: {v} 度 (90 + 180k 度) では定義されません"))
        }
        "tan" => Ok(v.to_radians().tan()),
        _ => return None,
    })
}
