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
