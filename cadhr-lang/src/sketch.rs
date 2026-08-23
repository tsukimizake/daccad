//! sketch DSL (`sketch .. in .. end`) と GUI sketch workspace の双方向連携。
//!
//! - [`model`] / [`model_from_source`]: forward eval。sketch ブロックから描画・
//!   ハンドル表示用の [`SketchModel`] を作る (manifold 不要・同期・軽量)。
//! - [`drag`]: 逆評価。ハンドルのドラッグ (新しい座標) を var / リテラルへの
//!   テキスト書き換えとして解決する。書き込み先の決定規則:
//!     - 座標式全体が符号付き Float リテラル → そのリテラル (匿名 var 扱い)
//!     - `var` 束縛への参照 → その定義リテラル
//!     - `let` 束縛への参照 → RHS を辿って var へ押し込む (導出値。
//!       `let t = b + 150.0` の t をドラッグすると b が書き換わる)。
//!       RHS に var が無い (`let y = 3.0` 等) 場合は書き込み不可
//!     - cross-sketch 参照 `A.f` → export 元 sketch の var / let に同じ規則を適用
//!       (`A.f` が var 由来なら書き込み、let 由来なら A のスコープで RHS を辿る)。
//!       `A.pt.x` / `.y` は export された点の該当軸の座標式へ押し込む。
//!       書き込むと参照している全 sketch のハンドルが連動する
//!     - 二項演算 → 書き込み可能な側がちょうど 1 つならそちらへ押し込む
//!       (もう一方は現在値で定数化)。両方可 / 両方不可なら拒否。
//!
//!   共有頂点 (junction) は構成する全ての座標式へ書き込む。軸ごとに独立で、
//!   片軸だけ固定されている場合は動かせる軸のみ適用し `pinned` として報告する。
//! - 構造編集 ([`add_point`] など): binding の挿入 / 削除と body record の更新。
//!
//! cross-sketch 参照の静的解決は [`Sketches`] が担う: モジュール内の全 top-level
//! sketch を参照の依存順 (topo 順) に評価し、循環は解決不能としてエラーにする
//! (runtime の依存順評価も循環を拒否するので一致する)。
//!
//! 将来の拘束ソルバー導入時は「新しい値の割り当てを決める」部分 (invert) を
//! 差し替え、リテラル書き換え ([`TextEdit`] / [`apply_edits`]) はそのまま使う。

use crate::diagnostic::Span;
use crate::geom::{MATH_FNS, eval_math_fn};
use crate::syntax::ast::*;
use crate::syntax::free_vars::free_var_names_in;
use crate::syntax::parse;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// 公開 model 型
// ---------------------------------------------------------------------------

/// GUI へ渡す sketch ブロックの評価済み形状。
#[derive(Clone, Debug, PartialEq)]
pub struct SketchModel {
    pub binding: String,
    /// sketch 式全体の span。
    pub span: Span,
    pub geoms: Vec<SketchGeom>,
    /// ハンドルごとの連動情報。
    pub links: Vec<HandleLink>,
}

/// ハンドル 1 つ分の連動情報。`leaves` はドラッグで書き込まれるリテラル葉の
/// ソース上の位置 (span start)。葉を共有するハンドル同士はドラッグで連動して動く。
#[derive(Clone, Debug, PartialEq)]
pub struct HandleLink {
    pub target: DragTarget,
    pub leaves: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SketchGeom {
    /// 参照点 (`p2`)。
    Point { name: String, pos: [f64; 2] },
    /// 名前付き線分 (`line`)。polygon から参照されていても列挙する。
    Segment {
        name: String,
        a: [f64; 2],
        b: [f64; 2],
    },
    /// 頂点列 (暗黙に閉じる)。
    Polygon { name: String, verts: Vec<[f64; 2]> },
    Circle {
        name: String,
        center: [f64; 2],
        radius: f64,
    },
}

impl SketchGeom {
    pub fn name(&self) -> &str {
        match self {
            SketchGeom::Point { name, .. }
            | SketchGeom::Segment { name, .. }
            | SketchGeom::Polygon { name, .. }
            | SketchGeom::Circle { name, .. } => name,
        }
    }
}

/// ドラッグ対象。`geom` は [`SketchModel::geoms`] の index。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragTarget {
    Point {
        geom: usize,
    },
    /// `end`: 0 = 始点, 1 = 終点。
    SegmentEnd {
        geom: usize,
        end: usize,
    },
    PolyVertex {
        geom: usize,
        vert: usize,
    },
    CircleCenter {
        geom: usize,
    },
    CircleRadius {
        geom: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragValue {
    Pos([f64; 2]),
    Radius(f64),
}

/// ドラッグ 1 ステップの結果。`source` が新しいソース全文。
#[derive(Clone, Debug)]
pub struct DragOutcome {
    pub source: String,
    pub model: SketchModel,
    /// 書き込みできず固定されたままの軸の説明。空なら全軸適用。
    pub pinned: Vec<String>,
}

/// ドラッグが一切適用できなかった理由。
#[derive(Clone, Debug, PartialEq)]
pub enum DragReject {
    /// 対象軸が読み取り専用 (座標式から書き込み可能な var に到達できない)。
    ReadOnly(String),
    /// 書き込み先が曖昧 (二項演算の両辺が書き込み可能)。
    Ambiguous(String),
    /// 係数 0 で逆評価できない。
    ZeroFactor(String),
    /// 複数の書き込みが同じリテラルに異なる値で衝突した。
    Conflict(String),
    /// sketch ブロックの構造が想定外 (validation を通っていない等)。
    Invalid(String),
}

impl DragReject {
    pub fn message(&self) -> &str {
        match self {
            DragReject::ReadOnly(m)
            | DragReject::Ambiguous(m)
            | DragReject::ZeroFactor(m)
            | DragReject::Conflict(m)
            | DragReject::Invalid(m) => m,
        }
    }
}

/// ソーステキストへの置換編集。
#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    pub span: Span,
    pub replacement: String,
}

/// 編集群を適用した新しいソースを返す。span は重複しないこと。
pub fn apply_edits(src: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by_key(|e| e.span.start);
    let mut out = src.to_string();
    for e in sorted.iter().rev() {
        out.replace_range(e.span.range(), &e.replacement);
    }
    out
}

// ---------------------------------------------------------------------------
// top-level sketch の収集 / export / 依存順 (sema::sketch と共有)
// ---------------------------------------------------------------------------

/// top-level の sketch binding 1 つ分 (body が `Expr::Sketch` の引数なし binding)。
pub(crate) struct TopSketch<'a> {
    pub name: &'a str,
    pub bindings: &'a [SketchBinding],
    pub body: &'a Expr,
    pub span: Span,
}

pub(crate) fn top_sketches(module: &Module) -> Vec<TopSketch<'_>> {
    module
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Value(v) if v.params.is_empty() => match &v.body {
                Expr::Sketch {
                    bindings,
                    body,
                    span,
                } => Some(TopSketch {
                    name: v.name.as_str(),
                    bindings,
                    body,
                    span: *span,
                }),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// body が `Expr::Sketch` そのものである引数なし top-level binding の名前一覧。
pub fn sketch_binding_names(module: &Module) -> Vec<String> {
    top_sketches(module)
        .iter()
        .map(|s| s.name.to_string())
        .collect()
}

fn find_block<'a>(
    module: &'a Module,
    binding: &str,
) -> Result<(&'a [SketchBinding], &'a Expr, Span), String> {
    // 同名の top-level binding が複数あるとどれを指すか決められない
    // (表示系の Sketches と同じ規則で操作を拒否する)。
    if module
        .decls
        .iter()
        .filter(|d| matches!(d, Decl::Value(v) if v.name == binding))
        .count()
        > 1
    {
        return Err(format!(
            "`{binding}` と同名の top-level binding が複数あります"
        ));
    }
    for d in &module.decls {
        if let Decl::Value(v) = d
            && v.name == binding
            && v.params.is_empty()
            && let Expr::Sketch {
                bindings,
                body,
                span,
            } = &v.body
        {
            return Ok((bindings, body, *span));
        }
    }
    Err(format!(
        "`{binding}` は sketch ブロックの binding ではありません"
    ))
}

/// body record の export (field 名 → 参照先 binding 名)。`{ f = name }` / punning
/// `{ name }` の Var field のみ拾う (form の妥当性は sema が検査する)。
/// 単一幾何名 body は export なし。
pub(crate) fn export_map<'a>(
    bindings: &'a [SketchBinding],
    body: &'a Expr,
) -> HashMap<&'a str, &'a str> {
    let names: HashSet<&str> = bindings.iter().map(|b| b.name.as_str()).collect();
    let mut out = HashMap::new();
    if let Expr::Record(fields, _) = body {
        for f in fields {
            if let Expr::Var {
                module: None, name, ..
            } = &f.value
                && names.contains(name.as_str())
            {
                out.insert(f.name.as_str(), name.as_str());
            }
        }
    }
    out
}

/// 式中の field 参照チェーン (`A.f` / `A.pt.x`) の (最内 receiver 名, 直後の field 名)
/// を集める。cross-sketch 参照の依存走査 ([`sketch_topo`]) と export 参照の検出
/// ([`remove_geom`]) に使う。
fn collect_field_refs<'a>(e: &'a Expr, out: &mut Vec<(&'a str, &'a str)>) {
    if let Expr::Field { receiver, name, .. } = e {
        let mut field = name.as_str();
        let mut base = receiver.as_ref();
        while let Expr::Field { receiver, name, .. } = base {
            field = name.as_str();
            base = receiver.as_ref();
        }
        match base {
            Expr::Var {
                module: None, name, ..
            } => out.push((name.as_str(), field)),
            other => collect_field_refs(other, out),
        }
        return;
    }
    match e {
        Expr::Var { .. } | Expr::Ctor { .. } | Expr::Lit(..) | Expr::Error(_) => {}
        Expr::List(items, _) => items.iter().for_each(|x| collect_field_refs(x, out)),
        Expr::Record(fields, _) => fields
            .iter()
            .for_each(|f| collect_field_refs(&f.value, out)),
        Expr::RecordUpdate { base, updates, .. } => {
            collect_field_refs(base, out);
            updates
                .iter()
                .for_each(|f| collect_field_refs(&f.value, out));
        }
        Expr::App { func, arg, .. } => {
            collect_field_refs(func, out);
            collect_field_refs(arg, out);
        }
        Expr::Lambda { body, .. } => collect_field_refs(body, out),
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .for_each(|b| collect_field_refs(&b.body, out));
            collect_field_refs(body, out);
        }
        Expr::Sketch { bindings, body, .. } => {
            bindings
                .iter()
                .for_each(|b| collect_field_refs(&b.body, out));
            collect_field_refs(body, out);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_field_refs(cond, out);
            collect_field_refs(then_branch, out);
            collect_field_refs(else_branch, out);
        }
        Expr::Case {
            scrutinee, arms, ..
        } => {
            collect_field_refs(scrutinee, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    collect_field_refs(g, out);
                }
                collect_field_refs(&a.body, out);
            }
        }
        Expr::BinOp { left, right, .. } => {
            collect_field_refs(left, out);
            collect_field_refs(right, out);
        }
        Expr::Negate(inner, _) => collect_field_refs(inner, out),
        Expr::Range { lo, hi, .. } => {
            collect_field_refs(lo, out);
            collect_field_refs(hi, out);
        }
        Expr::Field { .. } => unreachable!("上で処理済み"),
    }
}

/// binding 群が参照する sketch 名 (`names` に含まれるもの)。
fn sketch_deps<'a>(bindings: &'a [SketchBinding], names: &HashSet<&'a str>) -> HashSet<&'a str> {
    let mut refs = Vec::new();
    for b in bindings {
        collect_field_refs(&b.body, &mut refs);
    }
    refs.into_iter()
        .map(|(base, _)| base)
        .filter(|n| names.contains(n))
        .collect()
}

/// [`sketch_topo`] の結果。各 Vec は宣言順。
pub(crate) struct SketchOrder<'a> {
    /// 依存順 (foreign 参照が必ず先に評価済みになる順)。
    pub order: Vec<&'a str>,
    /// 参照循環の実メンバー (自己参照含む = 依存グラフで自分自身へ到達できるもの)。
    pub cycle: Vec<&'a str>,
    /// 循環に (推移的に) 依存していて評価順を決められない sketch。
    pub dependent: Vec<&'a str>,
}

/// sketch 間の依存グラフを解く。
pub(crate) fn sketch_topo<'a>(sketches: &[&TopSketch<'a>]) -> SketchOrder<'a> {
    let names: HashSet<&str> = sketches.iter().map(|s| s.name).collect();
    let deps: Vec<HashSet<&str>> = sketches
        .iter()
        .map(|s| sketch_deps(s.bindings, &names))
        .collect();
    let index: HashMap<&str, usize> = sketches
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name, i))
        .collect();
    let mut order: Vec<&'a str> = Vec::new();
    let mut done: HashSet<&str> = HashSet::new();
    let mut progressed = true;
    while progressed {
        progressed = false;
        for (i, s) in sketches.iter().enumerate() {
            if !done.contains(s.name) && deps[i].iter().all(|d| done.contains(d)) {
                done.insert(s.name);
                order.push(s.name);
                progressed = true;
            }
        }
    }
    // 解決できなかったもののうち、依存を辿って自分自身へ戻れるものが循環の実メンバー。
    let reaches_self = |start: &str| -> bool {
        let mut stack: Vec<&str> = vec![start];
        let mut seen: HashSet<&str> = HashSet::new();
        while let Some(n) = stack.pop() {
            for d in &deps[index[n]] {
                if *d == start {
                    return true;
                }
                if seen.insert(d) {
                    stack.push(d);
                }
            }
        }
        false
    };
    let mut cycle = Vec::new();
    let mut dependent = Vec::new();
    for s in sketches {
        if !done.contains(s.name) {
            if reaches_self(s.name) {
                cycle.push(s.name);
            } else {
                dependent.push(s.name);
            }
        }
    }
    SketchOrder {
        order,
        cycle,
        dependent,
    }
}

pub fn model(module: &Module, binding: &str) -> Result<SketchModel, String> {
    let sketches = Sketches::build_for(module, binding);
    let (scope, block) = sketches.entry(binding)?;
    let mut geoms = Vec::new();
    let mut links = Vec::new();
    for b in block.bindings {
        if b.kind != SketchBindKind::Bare {
            continue;
        }
        let gi = geoms.len();
        match sketches.geom_shape(scope, b)? {
            GeomShape::Point(slot) => {
                links.push(HandleLink {
                    target: DragTarget::Point { geom: gi },
                    leaves: sketches.slot_leaves(scope, &slot),
                });
                geoms.push(SketchGeom::Point {
                    name: b.name.clone(),
                    pos: slot.pos,
                });
            }
            GeomShape::Segment(a, bb) => {
                links.push(HandleLink {
                    target: DragTarget::SegmentEnd { geom: gi, end: 0 },
                    leaves: sketches.slot_leaves(scope, &a),
                });
                links.push(HandleLink {
                    target: DragTarget::SegmentEnd { geom: gi, end: 1 },
                    leaves: sketches.slot_leaves(scope, &bb),
                });
                geoms.push(SketchGeom::Segment {
                    name: b.name.clone(),
                    a: a.pos,
                    b: bb.pos,
                });
            }
            GeomShape::Polygon(slots) => {
                for (vi, s) in slots.iter().enumerate() {
                    links.push(HandleLink {
                        target: DragTarget::PolyVertex { geom: gi, vert: vi },
                        leaves: sketches.slot_leaves(scope, s),
                    });
                }
                geoms.push(SketchGeom::Polygon {
                    name: b.name.clone(),
                    verts: slots.iter().map(|s| s.pos).collect(),
                });
            }
            GeomShape::Circle(c) => {
                links.push(HandleLink {
                    target: DragTarget::CircleCenter { geom: gi },
                    leaves: c
                        .dst
                        .as_ref()
                        .map(|d| sketches.slot_leaves(scope, d))
                        .unwrap_or_default(),
                });
                links.push(HandleLink {
                    target: DragTarget::CircleRadius { geom: gi },
                    leaves: sketches.axis_leaves(scope, &c.radius.exprs, c.radius.value),
                });
                geoms.push(SketchGeom::Circle {
                    name: b.name.clone(),
                    center: c.center_pos,
                    radius: c.radius.value,
                });
            }
        }
    }
    Ok(SketchModel {
        binding: binding.to_string(),
        span: block.span,
        geoms,
        links,
    })
}

pub fn model_from_source(src: &str, binding: &str) -> Result<SketchModel, String> {
    let module = parse_or_msg(src)?;
    model(&module, binding)
}

fn parse_or_msg(src: &str) -> Result<Module, String> {
    parse::parse(src).map_err(|e| {
        e.first()
            .map(|d| d.message())
            .unwrap_or_else(|| "parse error".to_string())
    })
}

// ---------------------------------------------------------------------------
// 内部表現: 座標スロット (値 + 書き込み対象式の集合)
// ---------------------------------------------------------------------------

/// スカラー 1 つ分。`exprs` は junction を構成する全座標式 (通常 1 つ)。
#[derive(Clone)]
struct Slot<'a> {
    value: f64,
    exprs: Vec<&'a Expr>,
}

/// 2D 位置 1 つ分。
#[derive(Clone)]
struct PosSlot<'a> {
    pos: [f64; 2],
    xs: Vec<&'a Expr>,
    ys: Vec<&'a Expr>,
}

impl<'a> PosSlot<'a> {
    /// junction の合流。同じ式 (span 一致) は重複させない。
    fn merge(&mut self, other: PosSlot<'a>) {
        for e in other.xs {
            if !self.xs.iter().any(|x| x.span() == e.span()) {
                self.xs.push(e);
            }
        }
        for e in other.ys {
            if !self.ys.iter().any(|y| y.span() == e.span()) {
                self.ys.push(e);
            }
        }
    }
}

struct CircleShape<'a> {
    center_pos: [f64; 2],
    /// `translate2d` の dst 引数。無ければ原点固定で中心は動かせない。
    dst: Option<PosSlot<'a>>,
    /// `translate2d` の src の静的値 (中心 = dst - src)。
    src: [f64; 2],
    radius: Slot<'a>,
}

enum GeomShape<'a> {
    Point(PosSlot<'a>),
    Segment(PosSlot<'a>, PosSlot<'a>),
    Polygon(Vec<PosSlot<'a>>),
    Circle(CircleShape<'a>),
}

/// スカラー名の解決結果 ([`Sketches::scalar_binding`])。
enum ScalarBinding<'a> {
    /// 書き込み対象の `var` (RHS は符号付きリテラル)。`block` は所属 sketch。
    Var {
        block: &'a str,
        name: &'a str,
        body: &'a Expr,
    },
    /// 導出値の `let`。逆評価は RHS を辿って var へ押し込む。`block` は RHS のスコープ。
    Let { block: &'a str, body: &'a Expr },
    /// スカラーとして辿れない (幾何 binding / 未定義)。
    Opaque,
}

/// cross-sketch 参照 (`A.f` / `A.pt.x`) の解決結果 ([`Sketches::field_ref`])。
enum FieldRef<'a> {
    /// export されたスカラー (var / let binding)。
    Scalar {
        block: &'a str,
        binding: &'a SketchBinding,
    },
    /// export された点の座標軸。`expr` は `p2` の該当軸の引数式。
    PointAxis { block: &'a str, expr: &'a Expr },
}

/// sketch ブロック 1 つ分の束縛表と静的スカラー値。
struct Block<'a> {
    bindings: &'a [SketchBinding],
    by_name: HashMap<&'a str, &'a SketchBinding>,
    /// body record の export (field 名 → binding 名)。
    exports: HashMap<&'a str, &'a str>,
    /// var / let の静的値 (評価に成功したもののみ)。
    scalar_values: HashMap<&'a str, f64>,
    span: Span,
    /// スカラー束縛の評価で最初に出たエラー。ある場合このブロックは操作対象にできない。
    error: Option<String>,
}

/// モジュール内の top-level sketch の静的解決コンテキスト。cross-sketch 参照は
/// export 経由で他ブロックの束縛へジャンプする。ブロックは依存順 (topo 順) に
/// 評価するので、解決の再帰は依存順を遡る方向にしか進まず必ず停止する。
struct Sketches<'a> {
    blocks: HashMap<&'a str, Block<'a>>,
    /// 参照循環の実メンバー (自己参照含む)。宣言順。
    cycle: Vec<&'a str>,
    /// 循環に依存していて評価順を決められない sketch。宣言順。
    dependent: Vec<&'a str>,
    /// 同名の top-level sketch が複数ある名前 (どれを指すか決められないので操作拒否)。
    duplicated: HashSet<&'a str>,
}

impl<'a> Sketches<'a> {
    /// 全 top-level sketch を評価する ([`writeback_ambiguities`] のような全走査用)。
    fn build(module: &'a Module) -> Sketches<'a> {
        Self::build_impl(module, None)
    }

    /// `target` の推移的依存のみ静的評価する (GUI 操作用)。無関係な sketch の
    /// 評価コストを払わず、そのエラーにも影響されない。
    fn build_for(module: &'a Module, target: &str) -> Sketches<'a> {
        Self::build_impl(module, Some(target))
    }

    fn build_impl(module: &'a Module, target: Option<&str>) -> Sketches<'a> {
        let tops = top_sketches(module);
        // sketch 以外も含む全 top-level binding 名で重複を数える
        // (sketch と通常 binding の同名も解決先が定まらないため)。
        let mut duplicated: HashSet<&'a str> = HashSet::new();
        {
            let mut seen: HashSet<&str> = HashSet::new();
            for d in &module.decls {
                if let Decl::Value(v) = d
                    && !seen.insert(v.name.as_str())
                {
                    duplicated.insert(v.name.as_str());
                }
            }
        }
        // target 指定時は依存 DFS で評価対象を絞る。
        let selected: Vec<&TopSketch<'a>> = match target {
            None => tops.iter().collect(),
            Some(t) => {
                let names: HashSet<&str> = tops.iter().map(|s| s.name).collect();
                let mut keep: HashSet<&str> = HashSet::new();
                let mut stack: Vec<&str> = tops
                    .iter()
                    .filter(|s| s.name == t)
                    .map(|s| s.name)
                    .collect();
                while let Some(n) = stack.pop() {
                    if !keep.insert(n) {
                        continue;
                    }
                    for s in tops.iter().filter(|s| s.name == n) {
                        for d in sketch_deps(s.bindings, &names) {
                            if !keep.contains(d) {
                                stack.push(d);
                            }
                        }
                    }
                }
                tops.iter().filter(|s| keep.contains(s.name)).collect()
            }
        };
        let order = sketch_topo(&selected);
        // 解決可能なものは依存順、循環関与分は最後 (エラーを block へ記録するため評価は試みる)。
        let eval_order: Vec<&'a str> = order
            .order
            .iter()
            .chain(order.cycle.iter())
            .chain(order.dependent.iter())
            .copied()
            .collect();
        let by: HashMap<&str, &TopSketch<'a>> = selected.iter().map(|s| (s.name, *s)).collect();
        let mut sketches = Sketches {
            blocks: HashMap::new(),
            cycle: order.cycle,
            dependent: order.dependent,
            duplicated,
        };
        // ブロックの骨格 (束縛表と export) は全 sketch 分作る。スカラーの静的評価は
        // selected のみ、依存順に行う。
        for s in &tops {
            sketches.blocks.insert(
                s.name,
                Block {
                    bindings: s.bindings,
                    by_name: s.bindings.iter().map(|b| (b.name.as_str(), b)).collect(),
                    exports: export_map(s.bindings, s.body),
                    scalar_values: HashMap::new(),
                    span: s.span,
                    error: None,
                },
            );
        }
        for name in eval_order {
            let s = by[name];
            for b in s.bindings {
                if b.kind == SketchBindKind::Bare {
                    continue;
                }
                match sketches.eval_scalar(s.name, &b.body) {
                    Ok(v) if !v.is_finite() => {
                        let block = sketches.blocks.get_mut(s.name).expect("挿入済み");
                        block
                            .error
                            .get_or_insert(format!("`{}` が有限値になりません ({v})", b.name));
                    }
                    Ok(v) => {
                        let block = sketches.blocks.get_mut(s.name).expect("挿入済み");
                        block.scalar_values.insert(b.name.as_str(), v);
                    }
                    Err(e) => {
                        let block = sketches.blocks.get_mut(s.name).expect("挿入済み");
                        block.error.get_or_insert(e);
                    }
                }
            }
        }
        sketches
    }

    /// 名前が解決不能な場合の理由。
    fn unresolvable(&self, name: &str) -> Option<String> {
        if self.duplicated.contains(name) {
            return Some(format!(
                "`{name}` と同名の top-level binding が複数あります"
            ));
        }
        if self.cycle.iter().any(|n| *n == name) {
            return Some(format!(
                "`{name}` は sketch 間の参照循環に含まれています (循環: {})",
                self.cycle.join(", ")
            ));
        }
        if self.dependent.iter().any(|n| *n == name) {
            return Some(format!(
                "`{name}` は循環している sketch に依存しているため解決できません (循環: {})",
                self.cycle.join(", ")
            ));
        }
        None
    }

    /// GUI 操作の対象ブロックを引く。スカラー評価に失敗しているブロック・
    /// 循環に関与するブロック・同名重複のあるブロックは操作できない。
    fn entry(&self, binding: &str) -> Result<(&'a str, &Block<'a>), String> {
        if let Some(reason) = self.unresolvable(binding) {
            return Err(reason);
        }
        let Some((name, block)) = self.blocks.get_key_value(binding) else {
            return Err(format!(
                "`{binding}` は sketch ブロックの binding ではありません"
            ));
        };
        if let Some(e) = &block.error {
            return Err(e.clone());
        }
        Ok((*name, block))
    }

    fn block(&self, scope: &str) -> Result<&Block<'a>, String> {
        self.blocks
            .get(scope)
            .ok_or_else(|| format!("sketch `{scope}` が見つかりません"))
    }

    /// export された field の参照先 binding を引く。
    fn export_binding(
        &self,
        sk: &str,
        field: &str,
    ) -> Result<(&'a str, &'a SketchBinding), String> {
        if let Some(reason) = self.unresolvable(sk) {
            return Err(reason);
        }
        let Some((name, block)) = self.blocks.get_key_value(sk) else {
            return Err(format!("`{sk}` は同一モジュールの sketch ではありません"));
        };
        let Some(binding_name) = block.exports.get(field) else {
            return Err(format!("sketch `{sk}` は `{field}` を export していません"));
        };
        let b = block
            .by_name
            .get(binding_name)
            .ok_or_else(|| format!("`{sk}.{field}` の参照先が見つかりません"))?;
        Ok((*name, *b))
    }

    /// 参照位置 `at` で receiver 名 `sk` がブロック内束縛に shadow されているか。
    /// binding は逐次スコープなので、参照より前に完結した binding のみ shadow する
    /// (sema / runtime の名前解決と一致させる)。
    fn shadowed_receiver(&self, scope: &str, sk: &str, at: Span) -> Result<(), String> {
        if let Some(bl) = self.blocks.get(scope)
            && let Some(b) = bl.by_name.get(sk)
            && b.span.end <= at.start
        {
            return Err(format!(
                "`{sk}` はブロック内の束縛です (sketch 参照には使えません)"
            ));
        }
        Ok(())
    }

    /// `A.f` / `A.pt.x` 形の field 式を解決する (scope = 参照元ブロック)。
    fn field_ref(&self, scope: &str, e: &Expr) -> Result<FieldRef<'a>, String> {
        let Expr::Field {
            receiver,
            name: field,
            ..
        } = e
        else {
            return Err("field 参照ではありません".to_string());
        };
        match receiver.as_ref() {
            // `A.f` — export されたスカラー
            Expr::Var {
                module: None,
                name: sk,
                span,
            } => {
                self.shadowed_receiver(scope, sk, *span)?;
                let (block, b) = self.export_binding(sk, field)?;
                match b.kind {
                    SketchBindKind::Var | SketchBindKind::Let => {
                        Ok(FieldRef::Scalar { block, binding: b })
                    }
                    SketchBindKind::Bare => Err(format!(
                        "`{sk}.{field}` はスカラーではありません (点の座標は `.x` / `.y` で参照します)"
                    )),
                }
            }
            // `A.pt.x` — export された点の座標
            Expr::Field {
                receiver: inner,
                name: ptfield,
                ..
            } => {
                let Expr::Var {
                    module: None,
                    name: sk,
                    span,
                } = inner.as_ref()
                else {
                    return Err("field 参照が解釈できません".to_string());
                };
                self.shadowed_receiver(scope, sk, *span)?;
                let axis = match field.as_str() {
                    "x" => 0,
                    "y" => 1,
                    _ => return Err(format!("点の座標 field は x / y のみです (`.{field}`)")),
                };
                let (block, b) = self.export_binding(sk, ptfield)?;
                let (head, args) = app_spine(&b.body);
                if b.kind == SketchBindKind::Bare && head == Some("p2") && args.len() == 2 {
                    Ok(FieldRef::PointAxis {
                        block,
                        expr: args[axis],
                    })
                } else {
                    Err(format!("`{sk}.{ptfield}` は点ではありません"))
                }
            }
            _ => Err("field 参照が解釈できません".to_string()),
        }
    }

    /// スカラー名を var / let / 不透明に解決する (scope = ブロック名)。
    fn scalar_binding(&self, scope: &'a str, name: &str) -> ScalarBinding<'a> {
        match self.blocks.get(scope).and_then(|bl| bl.by_name.get(name)) {
            Some(b) if b.kind == SketchBindKind::Var => ScalarBinding::Var {
                block: scope,
                name: b.name.as_str(),
                body: &b.body,
            },
            Some(b) if b.kind == SketchBindKind::Let => ScalarBinding::Let {
                block: scope,
                body: &b.body,
            },
            _ => ScalarBinding::Opaque,
        }
    }

    /// スカラー式の静的評価 (scope = ブロック名)。
    fn eval_scalar(&self, scope: &str, e: &Expr) -> Result<f64, String> {
        match e {
            Expr::Lit(Lit::Float(v), _) => Ok(*v),
            Expr::Negate(inner, _) => Ok(-self.eval_scalar(scope, inner)?),
            Expr::BinOp {
                op, left, right, ..
            } => {
                let l = self.eval_scalar(scope, left)?;
                let r = self.eval_scalar(scope, right)?;
                match op {
                    BinOp::Add => Ok(l + r),
                    BinOp::Sub => Ok(l - r),
                    BinOp::Mul => Ok(l * r),
                    BinOp::Div => Ok(l / r),
                    _ => Err("スカラー式として評価できません".to_string()),
                }
            }
            Expr::Var {
                module: None, name, ..
            } => self
                .block(scope)?
                .scalar_values
                .get(name.as_str())
                .copied()
                .ok_or_else(|| format!("`{name}` はスカラーではありません")),
            Expr::Field { .. } => match self.field_ref(scope, e)? {
                FieldRef::Scalar { block, binding } => self
                    .block(block)?
                    .scalar_values
                    .get(binding.name.as_str())
                    .copied()
                    .ok_or_else(|| format!("`{}` の値が解決できません", binding.name)),
                FieldRef::PointAxis { block, expr } => self.eval_scalar(block, expr),
            },
            Expr::App { .. } => match app_spine(e) {
                (Some(f), args) if args.len() == 1 && MATH_FNS.contains(&f) => {
                    let v = self.eval_scalar(scope, args[0])?;
                    eval_math_fn(f, v).expect("MATH_FNS で判定済み")
                }
                _ => Err("スカラー式として評価できません".to_string()),
            },
            _ => Err("スカラー式として評価できません".to_string()),
        }
    }

    /// 点参照 (点 binding 名か `p2 x y`) をスロットに解決する。
    fn point_slot(&self, scope: &'a str, e: &'a Expr) -> Result<PosSlot<'a>, String> {
        if let Expr::Var {
            module: None, name, ..
        } = e
        {
            let b = self
                .block(scope)?
                .by_name
                .get(name.as_str())
                .ok_or_else(|| format!("未定義の名前 `{name}`"))?;
            return self.point_slot(scope, &b.body);
        }
        let (head, args) = app_spine(e);
        if head == Some("p2") && args.len() == 2 {
            let x = self.eval_scalar(scope, args[0])?;
            let y = self.eval_scalar(scope, args[1])?;
            Ok(PosSlot {
                pos: [x, y],
                xs: vec![args[0]],
                ys: vec![args[1]],
            })
        } else {
            Err("点参照が `p2 x y` の形ではありません".to_string())
        }
    }

    /// 線分参照 (線分 binding 名か `line a b`) を両端点スロットに解決する。
    fn segment_ends(
        &self,
        scope: &'a str,
        e: &'a Expr,
    ) -> Result<(PosSlot<'a>, PosSlot<'a>), String> {
        if let Expr::Var {
            module: None, name, ..
        } = e
        {
            let b = self
                .block(scope)?
                .by_name
                .get(name.as_str())
                .ok_or_else(|| format!("未定義の名前 `{name}`"))?;
            return self.segment_ends(scope, &b.body);
        }
        let (head, args) = app_spine(e);
        if head == Some("line") && args.len() == 2 {
            Ok((
                self.point_slot(scope, args[0])?,
                self.point_slot(scope, args[1])?,
            ))
        } else {
            Err("線分参照が `line a b` の形ではありません".to_string())
        }
    }

    fn geom_shape(&self, scope: &'a str, b: &'a SketchBinding) -> Result<GeomShape<'a>, String> {
        let e = &b.body;
        let (head, args) = app_spine(e);
        match head {
            Some("p2") if args.len() == 2 => Ok(GeomShape::Point(self.point_slot(scope, e)?)),
            Some("line") if args.len() == 2 => {
                let (a, bb) = self.segment_ends(scope, e)?;
                Ok(GeomShape::Segment(a, bb))
            }
            Some("polygon") if args.len() == 1 => {
                Ok(GeomShape::Polygon(self.polygon_slots(scope, args[0])?))
            }
            Some("circle") if args.len() == 1 => Ok(GeomShape::Circle(CircleShape {
                center_pos: [0.0, 0.0],
                dst: None,
                src: [0.0, 0.0],
                radius: Slot {
                    value: self.eval_scalar(scope, args[0])?,
                    exprs: vec![args[0]],
                },
            })),
            _ => {
                if let Expr::BinOp {
                    op: BinOp::ApplyR,
                    left,
                    right,
                    ..
                } = e
                {
                    let (lhead, largs) = app_spine(left);
                    let (rhead, rargs) = app_spine(right);
                    if lhead == Some("circle")
                        && largs.len() == 1
                        && rhead == Some("translate2d")
                        && rargs.len() == 2
                    {
                        let src = self.point_slot(scope, rargs[0])?;
                        let dst = self.point_slot(scope, rargs[1])?;
                        let center_pos = [dst.pos[0] - src.pos[0], dst.pos[1] - src.pos[1]];
                        return Ok(GeomShape::Circle(CircleShape {
                            center_pos,
                            src: src.pos,
                            dst: Some(dst),
                            radius: Slot {
                                value: self.eval_scalar(scope, largs[0])?,
                                exprs: vec![largs[0]],
                            },
                        }));
                    }
                }
                Err(format!("`{}` の右辺が幾何式として解釈できません", b.name))
            }
        }
    }

    /// polygon の頂点スロット列。line 形式は隣接線分の端点を junction として合流する。
    fn polygon_slots(&self, scope: &'a str, arg: &'a Expr) -> Result<Vec<PosSlot<'a>>, String> {
        match arg {
            // `polygon [line .., ...]` (線分列)
            Expr::List(items, _) => {
                let mut ends: Vec<(PosSlot<'a>, PosSlot<'a>)> = Vec::new();
                for it in items {
                    ends.push(self.segment_ends(scope, it)?);
                }
                let n = ends.len();
                let mut verts: Vec<PosSlot<'a>> = Vec::new();
                for i in 0..n {
                    // 頂点 i = seg[i] の始点 ∪ seg[i-1] の終点 (wrap)
                    let mut v = ends[i].0.clone();
                    if n > 1 {
                        v.merge(ends[(i + n - 1) % n].1.clone());
                    }
                    verts.push(v);
                }
                Ok(verts)
            }
            // `polygon (segments [p2 .., ...])` (点列)
            _ => {
                let (head, args) = app_spine(arg);
                if head == Some("segments") && args.len() == 1 {
                    if let Expr::List(items, _) = args[0] {
                        let mut verts = Vec::new();
                        for it in items {
                            verts.push(self.point_slot(scope, it)?);
                        }
                        Ok(verts)
                    } else {
                        Err("segments の引数がリストリテラルではありません".to_string())
                    }
                } else {
                    Err("polygon の引数が解釈できません".to_string())
                }
            }
        }
    }

    /// 部分木に書き込み可能な葉が含まれるか。`let` と cross-sketch 参照は導出値として
    /// RHS / export 先に再帰する (再帰はブロックの依存順を遡る方向なので停止する。
    /// ブロック内の不正な let 連鎖は [`Sketches::build`] の評価エラーが先に弾く)。
    fn contains_writable(&self, scope: &'a str, e: &Expr) -> bool {
        match e {
            Expr::Var {
                module: None, name, ..
            } => match self.scalar_binding(scope, name) {
                ScalarBinding::Var { .. } => true,
                ScalarBinding::Let { block, body } => self.contains_writable(block, body),
                ScalarBinding::Opaque => false,
            },
            Expr::Field { .. } => match self.field_ref(scope, e) {
                Ok(FieldRef::Scalar { block, binding }) => match binding.kind {
                    SketchBindKind::Var => true,
                    SketchBindKind::Let => self.contains_writable(block, &binding.body),
                    SketchBindKind::Bare => false,
                },
                // 点座標はリテラル葉そのものも書き込み対象 (点のドラッグと同じ扱い)
                Ok(FieldRef::PointAxis { block, expr }) => {
                    signed_lit_leaf(expr).is_some() || self.contains_writable(block, expr)
                }
                Err(_) => false,
            },
            Expr::Negate(inner, _) => self.contains_writable(scope, inner),
            Expr::BinOp { left, right, .. } => {
                self.contains_writable(scope, left) || self.contains_writable(scope, right)
            }
            _ => false,
        }
    }

    /// 座標式 `e` に目標値 `target` を流し、書き換えるリテラル葉と新値を返す。
    fn invert(&self, scope: &'a str, e: &Expr, target: f64) -> Result<Write<'a>, SolveErr> {
        // 座標式全体が符号付きリテラル → 匿名 var として書き込み可
        if let Some((cur, leaf)) = signed_lit_leaf(e) {
            return Ok(Write {
                leaf,
                value: target,
                var: None,
                current: cur,
            });
        }
        self.invert_inner(scope, e, target)
    }

    fn invert_inner(&self, scope: &'a str, e: &Expr, target: f64) -> Result<Write<'a>, SolveErr> {
        match e {
            Expr::Var {
                module: None, name, ..
            } => match self.scalar_binding(scope, name) {
                ScalarBinding::Var { block, name, body } => {
                    // var の RHS は符号付きリテラル (validation 保証)
                    let (cur, leaf) = signed_lit_leaf(body).ok_or(SolveErr::ReadOnly)?;
                    Ok(Write {
                        leaf,
                        value: target,
                        var: Some((block, name)),
                        current: cur,
                    })
                }
                // let は導出値: RHS を辿って依存先の var へ押し込む。
                ScalarBinding::Let { block, body } => self.invert_inner(block, body, target),
                ScalarBinding::Opaque => Err(SolveErr::ReadOnly),
            },
            Expr::Field { .. } => {
                match self.field_ref(scope, e).map_err(|_| SolveErr::ReadOnly)? {
                    FieldRef::Scalar { block, binding } => match binding.kind {
                        SketchBindKind::Var => {
                            let (cur, leaf) =
                                signed_lit_leaf(&binding.body).ok_or(SolveErr::ReadOnly)?;
                            Ok(Write {
                                leaf,
                                value: target,
                                var: Some((block, binding.name.as_str())),
                                current: cur,
                            })
                        }
                        SketchBindKind::Let => self.invert_inner(block, &binding.body, target),
                        SketchBindKind::Bare => Err(SolveErr::ReadOnly),
                    },
                    // 点座標: export 元の座標式へ押し込む (リテラルなら匿名書き込み)
                    FieldRef::PointAxis { block, expr } => self.invert(block, expr, target),
                }
            }
            Expr::Negate(inner, _) => self.invert_inner(scope, inner, -target),
            Expr::BinOp {
                op, left, right, ..
            } => {
                let lw = self.contains_writable(scope, left);
                let rw = self.contains_writable(scope, right);
                match (lw, rw) {
                    (true, true) => Err(SolveErr::Ambiguous),
                    (false, false) => Err(SolveErr::ReadOnly),
                    (true, false) => {
                        let rv = self
                            .eval_scalar(scope, right)
                            .map_err(|_| SolveErr::ReadOnly)?;
                        match op {
                            BinOp::Add => self.invert_inner(scope, left, target - rv),
                            BinOp::Sub => self.invert_inner(scope, left, target + rv),
                            BinOp::Mul if rv != 0.0 => self.invert_inner(scope, left, target / rv),
                            BinOp::Mul => Err(SolveErr::ZeroFactor),
                            BinOp::Div => self.invert_inner(scope, left, target * rv),
                            _ => Err(SolveErr::ReadOnly),
                        }
                    }
                    (false, true) => {
                        let lv = self
                            .eval_scalar(scope, left)
                            .map_err(|_| SolveErr::ReadOnly)?;
                        match op {
                            BinOp::Add => self.invert_inner(scope, right, target - lv),
                            BinOp::Sub => self.invert_inner(scope, right, lv - target),
                            BinOp::Mul if lv != 0.0 => self.invert_inner(scope, right, target / lv),
                            BinOp::Mul => Err(SolveErr::ZeroFactor),
                            // Div の右辺はリテラル (validation 保証) なので writable にならない
                            _ => Err(SolveErr::ReadOnly),
                        }
                    }
                }
            }
            _ => Err(SolveErr::ReadOnly),
        }
    }

    /// 軸 1 つ分のドラッグ書き込み先リテラル葉 (span start)。1 式でも書き込め
    /// ない場合 [`drag`] はその軸ごと固定するので、連動情報としても空を返す。
    fn axis_leaves(&self, scope: &'a str, exprs: &[&Expr], value: f64) -> Vec<usize> {
        let mut leaves = Vec::new();
        for e in exprs {
            match self.invert(scope, e, value) {
                Ok(w) => leaves.push(w.leaf.span.start),
                Err(_) => return Vec::new(),
            }
        }
        leaves
    }

    fn slot_leaves(&self, scope: &'a str, slot: &PosSlot) -> Vec<usize> {
        let mut leaves = self.axis_leaves(scope, &slot.xs, slot.pos[0]);
        leaves.extend(self.axis_leaves(scope, &slot.ys, slot.pos[1]));
        leaves
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SolveErr {
    ReadOnly,
    Ambiguous,
    ZeroFactor,
}

impl SolveErr {
    fn reason(self) -> &'static str {
        match self {
            SolveErr::ReadOnly => "書き込める var がありません",
            SolveErr::Ambiguous => "書き込み先が曖昧です (両辺に var があります)",
            SolveErr::ZeroFactor => "係数が 0 のため動かせません",
        }
    }

    fn into_reject(self, msg: String) -> DragReject {
        match self {
            SolveErr::ReadOnly => DragReject::ReadOnly(msg),
            SolveErr::Ambiguous => DragReject::Ambiguous(msg),
            SolveErr::ZeroFactor => DragReject::ZeroFactor(msg),
        }
    }
}

/// 書き換え対象のリテラル葉。
#[derive(Clone, Copy, Debug)]
struct Leaf {
    span: Span,
    /// `Negate(Lit)` 全体 (span が `-` を含む) か。
    negated: bool,
}

/// 逆評価 1 式分の結果。
#[derive(Clone, Copy, Debug)]
struct Write<'a> {
    leaf: Leaf,
    /// 葉へ書き込む新値。
    value: f64,
    /// 書き込み先 var の (所属 sketch, 名前)。None は座標式そのものがリテラル (匿名)。
    var: Option<(&'a str, &'a str)>,
    /// 葉の現在値 (符号込み)。
    current: f64,
}

/// var 名の表示形。操作対象ブロック外の var は `sketch名.var名` で修飾する。
fn var_display(scope: &str, var: Option<(&str, &str)>) -> Option<String> {
    var.map(|(block, name)| {
        if block == scope {
            name.to_string()
        } else {
            format!("{block}.{name}")
        }
    })
}

fn signed_lit_leaf(e: &Expr) -> Option<(f64, Leaf)> {
    match e {
        Expr::Lit(Lit::Float(v), span) => Some((
            *v,
            Leaf {
                span: *span,
                negated: false,
            },
        )),
        Expr::Negate(inner, span) => match inner.as_ref() {
            Expr::Lit(Lit::Float(v), _) => Some((
                -v,
                Leaf {
                    span: *span,
                    negated: true,
                },
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Float リテラルとして re-lex 可能な形に整形する (`.0` を保証)。
/// -0.0 は 0.0 に正規化する (`-0.0 < 0.0` が false のため括弧が付かず、
/// `-0.0` のまま埋め込むと引数位置で減算にパースされてしまう)。
fn fmt_float(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// 葉の置換テキスト。負数は元が `-lit` 全体でない限り括弧で包む
/// (関数適用の引数位置で減算に解釈されないように)。
fn fmt_leaf(v: f64, leaf: &Leaf) -> String {
    let s = fmt_float(v);
    if !leaf.negated && v < 0.0 {
        format!("({s})")
    } else {
        s
    }
}

/// 式の引数位置に埋め込む座標リテラル。負数は括弧で包む。
fn fmt_coord(v: f64) -> String {
    let s = fmt_float(v);
    if v < 0.0 { format!("({s})") } else { s }
}

// ---------------------------------------------------------------------------
// ハンドル → 軸解決 (drag / inspect / set_var_value で共有)
// ---------------------------------------------------------------------------

/// ハンドル 1 軸分の書き込み対象。
struct HandleAxis<'a> {
    label: &'static str,
    /// 書き込み対象の座標式 (junction は複数)。
    exprs: Vec<&'a Expr>,
    /// 幾何座標の現在値。
    display: f64,
    /// 幾何座標 → invert に流す値の補正 (circle 中心の dst = 中心 + src)。
    offset: f64,
    /// 構造的に書けない理由 (Some なら exprs は空)。
    readonly: Option<String>,
}

fn handle_axes<'a>(
    sketches: &Sketches<'a>,
    scope: &'a str,
    bare: &[&'a SketchBinding],
    target: DragTarget,
) -> Result<Vec<HandleAxis<'a>>, String> {
    let geom_of = |i: usize| -> Result<&'a SketchBinding, String> {
        bare.get(i)
            .copied()
            .ok_or_else(|| format!("幾何 index {i} が範囲外です"))
    };
    let pos_axes = |slot: PosSlot<'a>| -> Vec<HandleAxis<'a>> {
        vec![
            HandleAxis {
                label: "x",
                exprs: slot.xs,
                display: slot.pos[0],
                offset: 0.0,
                readonly: None,
            },
            HandleAxis {
                label: "y",
                exprs: slot.ys,
                display: slot.pos[1],
                offset: 0.0,
                readonly: None,
            },
        ]
    };
    match target {
        DragTarget::Point { geom } => {
            let GeomShape::Point(slot) = sketches.geom_shape(scope, geom_of(geom)?)? else {
                return Err("対象が点ではありません".into());
            };
            Ok(pos_axes(slot))
        }
        DragTarget::SegmentEnd { geom, end } => {
            let GeomShape::Segment(a, b) = sketches.geom_shape(scope, geom_of(geom)?)? else {
                return Err("対象が線分ではありません".into());
            };
            Ok(pos_axes(if end == 0 { a } else { b }))
        }
        DragTarget::PolyVertex { geom, vert } => {
            let GeomShape::Polygon(slots) = sketches.geom_shape(scope, geom_of(geom)?)? else {
                return Err("対象が polygon ではありません".into());
            };
            let slot = slots
                .into_iter()
                .nth(vert)
                .ok_or_else(|| format!("頂点 index {vert} が範囲外です"))?;
            Ok(pos_axes(slot))
        }
        DragTarget::CircleCenter { geom } => {
            let GeomShape::Circle(c) = sketches.geom_shape(scope, geom_of(geom)?)? else {
                return Err("対象が circle ではありません".into());
            };
            match c.dst {
                Some(dst) => Ok(vec![
                    HandleAxis {
                        label: "x",
                        exprs: dst.xs,
                        display: c.center_pos[0],
                        offset: c.src[0],
                        readonly: None,
                    },
                    HandleAxis {
                        label: "y",
                        exprs: dst.ys,
                        display: c.center_pos[1],
                        offset: c.src[1],
                        readonly: None,
                    },
                ]),
                None => {
                    let reason = "この circle は translate2d が無いため中心を動かせません";
                    Ok(["x", "y"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| HandleAxis {
                            label,
                            exprs: Vec::new(),
                            display: c.center_pos[i],
                            offset: 0.0,
                            readonly: Some(reason.to_string()),
                        })
                        .collect())
                }
            }
        }
        DragTarget::CircleRadius { geom } => {
            let GeomShape::Circle(c) = sketches.geom_shape(scope, geom_of(geom)?)? else {
                return Err("対象が circle ではありません".into());
            };
            Ok(vec![HandleAxis {
                label: "半径",
                exprs: c.radius.exprs,
                display: c.radius.value,
                offset: 0.0,
                readonly: None,
            }])
        }
    }
}

// ---------------------------------------------------------------------------
// 書き戻し検査 (CLI warning 用)
// ---------------------------------------------------------------------------

/// [`writeback_ambiguities`] の報告 1 件。`span` は曖昧な座標式の位置。
#[derive(Clone, Debug, PartialEq)]
pub struct WritebackWarning {
    pub span: Span,
    pub message: String,
}

/// 全 sketch ブロックのハンドルを走査し、書き戻し先が曖昧 (二項演算の両辺に var) で
/// ドラッグが拒否される軸を報告する。読み取り専用の軸は意図的な凍結 (リテラル let 等)
/// がありうるので報告しない。junction で同じ座標式を共有するハンドルは 1 回だけ数える。
pub fn writeback_ambiguities(module: &Module) -> Vec<WritebackWarning> {
    let mut out = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let sketches = Sketches::build(module);
    for binding in sketch_binding_names(module) {
        let Ok((scope, block)) = sketches.entry(&binding) else {
            continue;
        };
        let bare: Vec<&SketchBinding> = block
            .bindings
            .iter()
            .filter(|b| b.kind == SketchBindKind::Bare)
            .collect();
        for (gi, b) in bare.iter().enumerate() {
            let Ok(shape) = sketches.geom_shape(scope, b) else {
                continue;
            };
            let targets: Vec<(DragTarget, String)> = match shape {
                GeomShape::Point(_) => vec![(DragTarget::Point { geom: gi }, b.name.clone())],
                GeomShape::Segment(..) => vec![
                    (
                        DragTarget::SegmentEnd { geom: gi, end: 0 },
                        format!("{} 始点", b.name),
                    ),
                    (
                        DragTarget::SegmentEnd { geom: gi, end: 1 },
                        format!("{} 終点", b.name),
                    ),
                ],
                GeomShape::Polygon(slots) => (0..slots.len())
                    .map(|vi| {
                        (
                            DragTarget::PolyVertex { geom: gi, vert: vi },
                            format!("{} 頂点{}", b.name, vi),
                        )
                    })
                    .collect(),
                GeomShape::Circle(_) => vec![
                    (
                        DragTarget::CircleCenter { geom: gi },
                        format!("{} 中心", b.name),
                    ),
                    (DragTarget::CircleRadius { geom: gi }, b.name.clone()),
                ],
            };
            for (target, desc) in targets {
                let Ok(axes) = handle_axes(&sketches, scope, &bare, target) else {
                    continue;
                };
                for axis in axes {
                    if axis.readonly.is_some() || axis.exprs.is_empty() {
                        continue;
                    }
                    if let Err(SolveErr::Ambiguous) = axis_writes(&sketches, scope, &axis) {
                        let span = axis.exprs[0].span();
                        if !seen.insert((span.start, span.end)) {
                            continue;
                        }
                        out.push(WritebackWarning {
                            span,
                            message: format!(
                                "{binding}: {desc} の {} は{}",
                                axis.label,
                                SolveErr::Ambiguous.reason()
                            ),
                        });
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// drag (逆評価)
// ---------------------------------------------------------------------------

pub fn drag(
    src: &str,
    binding: &str,
    target: DragTarget,
    value: DragValue,
) -> Result<DragOutcome, DragReject> {
    match value {
        DragValue::Pos(p) if !(p[0].is_finite() && p[1].is_finite()) => {
            return Err(DragReject::Invalid("座標が有限値ではありません".into()));
        }
        DragValue::Radius(r) if !r.is_finite() => {
            return Err(DragReject::Invalid("半径が有限値ではありません".into()));
        }
        _ => {}
    }
    let module = parse_or_msg(src).map_err(DragReject::Invalid)?;
    let sketches = Sketches::build_for(&module, binding);
    let (scope, block) = sketches.entry(binding).map_err(DragReject::Invalid)?;
    let bare: Vec<&SketchBinding> = block
        .bindings
        .iter()
        .filter(|b| b.kind == SketchBindKind::Bare)
        .collect();

    let axes = handle_axes(&sketches, scope, &bare, target).map_err(DragReject::Invalid)?;
    // 軸ごとの目標値 (幾何座標)。
    let goals: Vec<f64> = match value {
        DragValue::Pos(p) if axes.len() == 2 => vec![p[0], p[1]],
        DragValue::Radius(r) if axes.len() == 1 => vec![r],
        _ => {
            return Err(DragReject::Invalid(
                "ドラッグ対象と値の種類が一致しません".into(),
            ));
        }
    };

    // 軸ごとに独立して逆評価。軸内は全式が書けなければその軸ごと固定。
    let mut writes: Vec<(Leaf, f64)> = Vec::new();
    let mut pinned: Vec<String> = Vec::new();
    let mut first_err: Option<SolveErr> = None;
    for (axis, goal) in axes.into_iter().zip(goals) {
        if let Some(reason) = axis.readonly {
            // 構造的理由は軸をまたいで同文なので重複させない
            if !pinned.contains(&reason) {
                pinned.push(reason);
            }
            first_err.get_or_insert(SolveErr::ReadOnly);
            continue;
        }
        let tval = goal + axis.offset;
        let mut axis_writes: Vec<(Leaf, f64)> = Vec::new();
        let mut axis_err: Option<SolveErr> = if axis.exprs.is_empty() {
            Some(SolveErr::ReadOnly)
        } else {
            None
        };
        for e in axis.exprs {
            match sketches.invert(scope, e, tval) {
                Ok(w) => axis_writes.push((w.leaf, w.value)),
                Err(err) => {
                    axis_err = Some(err);
                    break;
                }
            }
        }
        match axis_err {
            None => writes.extend(axis_writes),
            Some(err) => {
                pinned.push(format!("{} は{}", axis.label, err.reason()));
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
    }
    if writes.is_empty() {
        let err = first_err.unwrap_or(SolveErr::ReadOnly);
        return Err(err.into_reject(pinned.join(" / ")));
    }

    // 同一葉への書き込みを整理: 同値は dedupe、異値は衝突。
    let mut merged: Vec<(Leaf, f64)> = Vec::new();
    for (leaf, v) in writes {
        if let Some((_, prev)) = merged.iter().find(|(l, _)| l.span == leaf.span) {
            if (prev - v).abs() > f64::EPSILON {
                return Err(DragReject::Conflict(
                    "同じ値に異なる座標を書き込もうとしています (x と y が同じ var を共有)".into(),
                ));
            }
        } else {
            merged.push((leaf, v));
        }
    }

    let edits: Vec<TextEdit> = merged
        .iter()
        .map(|(leaf, v)| TextEdit {
            span: leaf.span,
            replacement: fmt_leaf(*v, leaf),
        })
        .collect();
    let source = apply_edits(src, &edits);
    let model = model_from_source(&source, binding).map_err(DragReject::Invalid)?;
    Ok(DragOutcome {
        source,
        model,
        pinned,
    })
}

// ---------------------------------------------------------------------------
// inspect / 数値入力による var 書き込み
// ---------------------------------------------------------------------------

/// 選択したハンドルの軸ごとの編集情報 (GUI のインスペクタ表示用)。
#[derive(Clone, Debug, PartialEq)]
pub struct HandleInspect {
    pub axes: Vec<AxisInspect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AxisInspect {
    /// "x" / "y" / "半径"。
    pub label: String,
    /// 幾何座標の現在値。
    pub value: f64,
    /// ドラッグ / 数値編集で書き込まれる var (匿名リテラル含む)。
    /// index は [`set_var_value`] の `write` に対応する。
    pub writes: Vec<VarInspect>,
    /// 書けない理由 (Some のとき writes は空)。
    pub readonly: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarInspect {
    /// var 名。None は座標式そのものがリテラル (匿名)。
    pub name: Option<String>,
    /// 書き込み先リテラルの現在値。
    pub value: f64,
}

/// 軸 1 つ分の書き込み先葉を inspect と同じ順序 (dedupe 済み) で集める。
fn axis_writes<'a>(
    sketches: &Sketches<'a>,
    scope: &'a str,
    axis: &HandleAxis<'a>,
) -> Result<Vec<Write<'a>>, SolveErr> {
    let mut out: Vec<Write<'a>> = Vec::new();
    for e in &axis.exprs {
        let w = sketches.invert(scope, e, axis.display + axis.offset)?;
        if !out.iter().any(|p| p.leaf.span == w.leaf.span) {
            out.push(w);
        }
    }
    Ok(out)
}

/// ハンドル 1 つ分の書き込み先 var 情報を返す。
pub fn inspect(src: &str, binding: &str, target: DragTarget) -> Result<HandleInspect, String> {
    let module = parse_or_msg(src)?;
    let sketches = Sketches::build_for(&module, binding);
    let (scope, block) = sketches.entry(binding)?;
    let bare: Vec<&SketchBinding> = block
        .bindings
        .iter()
        .filter(|b| b.kind == SketchBindKind::Bare)
        .collect();
    let mut axes = Vec::new();
    for axis in handle_axes(&sketches, scope, &bare, target)? {
        let (writes, readonly) = if let Some(reason) = axis.readonly.clone() {
            (Vec::new(), Some(reason))
        } else if axis.exprs.is_empty() {
            (Vec::new(), Some(SolveErr::ReadOnly.reason().to_string()))
        } else {
            match axis_writes(&sketches, scope, &axis) {
                Ok(ws) => (
                    ws.into_iter()
                        .map(|w| VarInspect {
                            name: var_display(scope, w.var),
                            value: w.current,
                        })
                        .collect(),
                    None,
                ),
                Err(err) => (Vec::new(), Some(err.reason().to_string())),
            }
        };
        axes.push(AxisInspect {
            label: axis.label.to_string(),
            value: axis.display,
            writes,
            readonly,
        });
    }
    Ok(HandleInspect { axes })
}

/// [`inspect`] の `axes[axis].writes[write]` の書き込み先リテラルへ `value` を書き込む。
pub fn set_var_value(
    src: &str,
    binding: &str,
    target: DragTarget,
    axis: usize,
    write: usize,
    value: f64,
) -> Result<DragOutcome, String> {
    if !value.is_finite() {
        return Err("値が有限ではありません".to_string());
    }
    let module = parse_or_msg(src)?;
    let sketches = Sketches::build_for(&module, binding);
    let (scope, block) = sketches.entry(binding)?;
    let bare: Vec<&SketchBinding> = block
        .bindings
        .iter()
        .filter(|b| b.kind == SketchBindKind::Bare)
        .collect();
    let ax = handle_axes(&sketches, scope, &bare, target)?
        .into_iter()
        .nth(axis)
        .ok_or_else(|| format!("軸 index {axis} が範囲外です"))?;
    if let Some(reason) = ax.readonly.clone() {
        return Err(reason);
    }
    let leaf = axis_writes(&sketches, scope, &ax)
        .map_err(|e| e.reason().to_string())?
        .into_iter()
        .nth(write)
        .ok_or_else(|| format!("書き込み先 index {write} が範囲外です"))?
        .leaf;
    let edits = [TextEdit {
        span: leaf.span,
        replacement: fmt_leaf(value, &leaf),
    }];
    let source = apply_edits(src, &edits);
    let model = model_from_source(&source, binding)?;
    Ok(DragOutcome {
        source,
        model,
        pinned: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// 構造編集
// ---------------------------------------------------------------------------

/// 既存名と衝突しない `prefix{n}` を返す。
fn fresh_name(prefix: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut n = 1;
    loop {
        let name = format!("{prefix}{n}");
        if !taken(&name) {
            return name;
        }
        n += 1;
    }
}

/// `span.start` を含む行のインデント文字列。
fn line_indent(src: &str, at: usize) -> String {
    let line_start = src[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    src[line_start..at]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect()
}

/// body record に field を足す編集。field は punning (`{ name }`) で書く。
fn body_add_field(body: &Expr, name: &str) -> Result<TextEdit, String> {
    match body {
        Expr::Record(fields, span) => match fields.last() {
            Some(last) => Ok(TextEdit {
                span: Span::new(last.span.end, last.span.end),
                replacement: format!(", {name}"),
            }),
            None => Ok(TextEdit {
                span: *span,
                replacement: format!("{{ {name} }}"),
            }),
        },
        Expr::Var {
            name: old, span, ..
        } => Ok(TextEdit {
            span: *span,
            replacement: format!("{{ {old}, {name} }}"),
        }),
        _ => Err("body が record か幾何名ではありません".to_string()),
    }
}

/// 新しい bare binding を末尾に挿入し body record に追加する。
/// 戻り値は (新ソース, 新 binding 名)。
fn insert_geom(
    src: &str,
    binding: &str,
    prefix: &str,
    rhs: &str,
) -> Result<(String, String), String> {
    let module = parse_or_msg(src)?;
    let (bindings, body, span) = find_block(&module, binding)?;
    // 生成名は sketch binding 名とも衝突させない (同名の局所 binding を作ると、
    // このブロックからの `名前.field` 参照が壊れるため。factor_vars と同じ規則)。
    let top_names: HashSet<&str> = top_sketches(&module).iter().map(|s| s.name).collect();
    let name = fresh_name(prefix, |n| {
        bindings.iter().any(|b| b.name == n) || top_names.contains(n)
    });
    // 挿入位置: 末尾 binding の直後。binding が 0 個なら `sketch` キーワード直後に
    // 1 段深いインデントで入れる。
    let insert = match bindings.last() {
        Some(last) => TextEdit {
            span: Span::new(last.span.end, last.span.end),
            replacement: format!("\n{}{name} = {rhs}", line_indent(src, last.span.start)),
        },
        None => {
            let after_kw = span.start + "sketch".len();
            TextEdit {
                span: Span::new(after_kw, after_kw),
                replacement: format!("\n{}    {name} = {rhs}", line_indent(src, span.start)),
            }
        }
    };
    let edits = vec![insert, body_add_field(body, &name)?];
    Ok((apply_edits(src, &edits), name))
}

/// 戻り値は (新ソース, 追加された binding 名)。
pub fn add_point(src: &str, binding: &str, pos: [f64; 2]) -> Result<(String, String), String> {
    let rhs = format!("p2 {} {}", fmt_coord(pos[0]), fmt_coord(pos[1]));
    insert_geom(src, binding, "pt", &rhs)
}

/// 戻り値は (新ソース, 追加された binding 名)。
pub fn add_circle(
    src: &str,
    binding: &str,
    center: [f64; 2],
    radius: f64,
) -> Result<(String, String), String> {
    let rhs = if center == [0.0, 0.0] {
        format!("circle {}", fmt_coord(radius))
    } else {
        format!(
            "circle {} |> translate2d (p2 0.0 0.0) (p2 {} {})",
            fmt_coord(radius),
            fmt_coord(center[0]),
            fmt_coord(center[1])
        )
    };
    insert_geom(src, binding, "circ", &rhs)
}

/// 戻り値は (新ソース, 追加された binding 名)。
pub fn add_polygon(
    src: &str,
    binding: &str,
    verts: &[[f64; 2]],
) -> Result<(String, String), String> {
    if verts.len() < 2 {
        return Err("polygon には 2 点以上必要です".to_string());
    }
    let pts = verts
        .iter()
        .map(|p| format!("p2 {} {}", fmt_coord(p[0]), fmt_coord(p[1])))
        .collect::<Vec<_>>()
        .join(", ");
    let rhs = format!("polygon (segments [{pts}])");
    insert_geom(src, binding, "poly", &rhs)
}

/// `polygon (segments [...])` 形式の polygon の末尾に頂点を足す。
pub fn append_polygon_vertex(
    src: &str,
    binding: &str,
    geom_name: &str,
    pos: [f64; 2],
) -> Result<String, String> {
    let module = parse_or_msg(src)?;
    let (bindings, _body, _span) = find_block(&module, binding)?;
    let b = bindings
        .iter()
        .find(|b| b.kind == SketchBindKind::Bare && b.name == geom_name)
        .ok_or_else(|| format!("幾何 `{geom_name}` が見つかりません"))?;
    let (head, args) = app_spine(&b.body);
    if head != Some("polygon") || args.len() != 1 {
        return Err(format!("`{geom_name}` は polygon ではありません"));
    }
    let (h2, a2) = app_spine(args[0]);
    let list = match (h2, a2.as_slice()) {
        (Some("segments"), [Expr::List(items, lspan)]) => (items, *lspan),
        _ => {
            return Err(format!(
                "`{geom_name}` は `polygon (segments [...])` 形式ではないため頂点追加できません"
            ));
        }
    };
    let txt = format!("p2 {} {}", fmt_coord(pos[0]), fmt_coord(pos[1]));
    let edit = match list.0.last() {
        Some(last) => TextEdit {
            span: Span::new(last.span().end, last.span().end),
            replacement: format!(", {txt}"),
        },
        None => TextEdit {
            span: Span::new(list.1.start + 1, list.1.start + 1),
            replacement: txt,
        },
    };
    Ok(apply_edits(src, &[edit]))
}

/// 幾何 binding とその body field を削除する。
pub fn remove_geom(src: &str, binding: &str, geom_name: &str) -> Result<String, String> {
    let module = parse_or_msg(src)?;
    let (bindings, body, _span) = find_block(&module, binding)?;
    let b = bindings
        .iter()
        .find(|b| b.name == geom_name)
        .ok_or_else(|| format!("`{geom_name}` が見つかりません"))?;
    // 他の束縛から参照されていたら削除できない (点や線分が他形状の一部の場合)。
    for other in bindings.iter().filter(|o| o.name != geom_name) {
        let mut bound = HashSet::new();
        let mut refs = HashSet::new();
        free_var_names_in(&other.body, &mut bound, &mut refs);
        if refs.contains(geom_name) {
            return Err(format!(
                "`{geom_name}` は `{}` から参照されているため削除できません",
                other.name
            ));
        }
    }
    // export されている場合、モジュール内 (他 sketch・関数型層) の `binding.field`
    // 参照が残っていたら削除できない。
    let exported: Vec<&str> = export_map(bindings, body)
        .into_iter()
        .filter(|(_, bn)| *bn == geom_name)
        .map(|(f, _)| f)
        .collect();
    if !exported.is_empty() {
        for d in &module.decls {
            let (referrer, decl_body) = match d {
                Decl::Value(v) if v.name != binding => (v.name.as_str(), &v.body),
                Decl::Slider(s) => (s.param.as_str(), &s.body),
                _ => continue,
            };
            let mut refs = Vec::new();
            collect_field_refs(decl_body, &mut refs);
            if let Some((_, f)) = refs
                .iter()
                .find(|(base, f)| *base == binding && exported.contains(f))
            {
                return Err(format!(
                    "`{geom_name}` は `{referrer}` から `{binding}.{f}` で参照されているため削除できません"
                ));
            }
        }
    }
    let mut edits: Vec<TextEdit> = Vec::new();
    // binding 行の削除 (行頭〜行末の改行まで)
    let line_start = src[..b.span.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = src[b.span.end..]
        .find('\n')
        .map(|i| b.span.end + i + 1)
        .unwrap_or(src.len());
    edits.push(TextEdit {
        span: Span::new(line_start, line_end),
        replacement: String::new(),
    });
    // body field の削除
    match body {
        Expr::Record(fields, span) => {
            let idx = fields
                .iter()
                .position(|f| matches!(&f.value, Expr::Var { name, .. } if name == geom_name));
            if let Some(i) = idx {
                let edit = if fields.len() == 1 {
                    TextEdit {
                        span: *span,
                        replacement: "{}".to_string(),
                    }
                } else if i == 0 {
                    TextEdit {
                        span: Span::new(fields[0].span.start, fields[1].span.start),
                        replacement: String::new(),
                    }
                } else {
                    TextEdit {
                        span: Span::new(fields[i - 1].span.end, fields[i].span.end),
                        replacement: String::new(),
                    }
                };
                edits.push(edit);
            }
        }
        Expr::Var { name, span, .. } if name == geom_name => {
            edits.push(TextEdit {
                span: *span,
                replacement: "{}".to_string(),
            });
        }
        _ => {}
    }
    Ok(apply_edits(src, &edits))
}

/// 座標式のうちリテラル葉だけを出現順で集める。junction などで共有された
/// 同一 span は 1 回だけ数える。
fn collect_lit_leaves(
    exprs: &[&Expr],
    out: &mut Vec<(Span, f64)>,
    seen: &mut HashSet<(usize, usize)>,
) {
    for e in exprs {
        if let Some((v, leaf)) = signed_lit_leaf(e)
            && seen.insert((leaf.span.start, leaf.span.end))
        {
            out.push((leaf.span, v));
        }
    }
}

/// 2 回以上現れる同値の座標リテラルを軸ごとに `var xN` / `var yN` へまとめる。
/// 対象は座標式全体がリテラルであるもののみ。既に var / 式になっている座標、
/// circle の半径と translate2d の src は対象外。まとめた座標は以後ドラッグで連動する。
/// x と y は値が同じでも別の var にする (共有すると斜めドラッグが衝突拒否されるため)。
pub fn factor_vars(src: &str, binding: &str) -> Result<String, String> {
    let module = parse_or_msg(src)?;
    let sketches = Sketches::build_for(&module, binding);
    let (scope, block) = sketches.entry(binding)?;
    let bindings = block.bindings;

    // 軸ごと (0 = x, 1 = y) の座標リテラルを出現順で集める
    let mut leaves: [Vec<(Span, f64)>; 2] = [Vec::new(), Vec::new()];
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for b in bindings.iter().filter(|b| b.kind == SketchBindKind::Bare) {
        let slots: Vec<PosSlot> = match sketches.geom_shape(scope, b)? {
            GeomShape::Point(s) => vec![s],
            GeomShape::Segment(a, b) => vec![a, b],
            GeomShape::Polygon(v) => v,
            GeomShape::Circle(c) => c.dst.into_iter().collect(),
        };
        for s in &slots {
            collect_lit_leaves(&s.xs, &mut leaves[0], &mut seen);
            collect_lit_leaves(&s.ys, &mut leaves[1], &mut seen);
        }
    }

    // 生成名は既存 binding と衝突させない。sketch binding 名も除外する
    // (同名の局所 binding を作ると、このブロックからの `名前.field` 参照が壊れるため)。
    let mut taken: HashSet<String> = bindings.iter().map(|b| b.name.clone()).collect();
    taken.extend(sketches.blocks.keys().map(|n| n.to_string()));
    let mut decls: Vec<(String, f64)> = Vec::new();
    let mut edits: Vec<TextEdit> = Vec::new();
    for (axis, prefix) in [(0, "x"), (1, "y")] {
        // 値ごとにグループ化 (初出順)。ビット比較なのでリテラル同士なら厳密一致。
        let mut groups: Vec<(u64, Vec<Span>)> = Vec::new();
        for (span, v) in &leaves[axis] {
            let bits = v.to_bits();
            match groups.iter_mut().find(|(b, _)| *b == bits) {
                Some((_, spans)) => spans.push(*span),
                None => groups.push((bits, vec![*span])),
            }
        }
        for (bits, spans) in groups {
            if spans.len() < 2 {
                continue;
            }
            let name = fresh_name(prefix, |n| taken.contains(n));
            taken.insert(name.clone());
            decls.push((name.clone(), f64::from_bits(bits)));
            for span in spans {
                edits.push(TextEdit {
                    span,
                    replacement: name.clone(),
                });
            }
        }
    }
    if edits.is_empty() {
        return Err("まとめられる重複座標がありません".to_string());
    }

    // var 宣言は前方参照禁止のため先頭の binding の直前に挿入する
    let first = bindings
        .first()
        .ok_or_else(|| "sketch ブロックに束縛がありません".to_string())?;
    let indent = line_indent(src, first.span.start);
    let mut header = String::new();
    for (name, v) in &decls {
        header.push_str(&format!("var {name} = {}\n{indent}", fmt_float(*v)));
    }
    edits.push(TextEdit {
        span: Span::new(first.span.start, first.span.start),
        replacement: header,
    });
    Ok(apply_edits(src, &edits))
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SRC_SEGMENTS: &str = "sk =\n    sketch\n        var x1 = 0.0\n        var x2 = 4.0\n        let y2 = 3.0\n        poly1 = polygon (segments [p2 x1 0.0, p2 x2 0.0, p2 x2 y2, p2 x1 y2])\n    in\n    { poly1 = poly1 }\n    end\n";

    const SRC_LINES: &str = "sk =\n    sketch\n        v1 = p2 0.0 0.0\n        v2 = p2 4.0 0.0\n        v3 = p2 4.0 3.0\n        l1 = line v1 v2\n        poly1 = polygon [l1, line v2 v3, line v3 v1]\n    in\n    { poly1 = poly1 }\n    end\n";

    fn model_of(src: &str) -> SketchModel {
        model_from_source(src, "sk").expect("model")
    }

    #[test]
    fn model_from_segments_polygon() {
        let m = model_of(SRC_SEGMENTS);
        assert_eq!(m.geoms.len(), 1);
        let SketchGeom::Polygon { name, verts } = &m.geoms[0] else {
            panic!("expected polygon: {:?}", m.geoms[0]);
        };
        assert_eq!(name, "poly1");
        assert_eq!(verts, &vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
    }

    #[test]
    fn model_from_line_polygon() {
        let m = model_of(SRC_LINES);
        // v1..v3 (点) + l1 (線分) + poly1
        assert_eq!(m.geoms.len(), 5);
        let SketchGeom::Polygon { verts, .. } = &m.geoms[4] else {
            panic!("expected polygon");
        };
        assert_eq!(verts, &vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]]);
    }

    #[test]
    fn model_point_with_math_fns() {
        let src = "sk =\n    sketch\n        var x = 3.0\n        let y = sqrt (x * x + 16.0)\n        pt = p2 (cos 60.0) y\n    in\n    { pt = pt }\n    end\n";
        let m = model_of(src);
        let SketchGeom::Point { pos, .. } = &m.geoms[0] else {
            panic!("expected point");
        };
        assert!((pos[0] - 0.5).abs() < 1e-12, "cos 60 = 0.5, got {}", pos[0]);
        assert!((pos[1] - 5.0).abs() < 1e-12, "sqrt 25 = 5, got {}", pos[1]);
        // sqrt を通る座標は逆評価できないので leaf は空 (軸固定) になる
        assert!(leaves_of(&m, DragTarget::Point { geom: 0 }).is_empty());
    }

    #[test]
    fn model_circle_translated() {
        let src = "sk =\n    sketch\n        var r = 2.0\n        circ1 = circle r |> translate2d (p2 1.0 1.0) (p2 4.0 6.0)\n    in\n    { circ1 = circ1 }\n    end\n";
        let m = model_of(src);
        let SketchGeom::Circle { center, radius, .. } = &m.geoms[0] else {
            panic!("expected circle");
        };
        assert_eq!(*center, [3.0, 5.0]);
        assert_eq!(*radius, 2.0);
    }

    fn leaves_of(m: &SketchModel, t: DragTarget) -> Vec<usize> {
        m.links
            .iter()
            .find(|l| l.target == t)
            .unwrap_or_else(|| panic!("no link for {t:?}"))
            .leaves
            .clone()
    }

    fn shares_leaf(m: &SketchModel, a: DragTarget, b: DragTarget) -> bool {
        let la = leaves_of(m, a);
        leaves_of(m, b).iter().any(|l| la.contains(l))
    }

    #[test]
    fn links_shared_var_couples_vertices() {
        let m = model_of(SRC_SEGMENTS);
        // 頂点 0 と 3 は x1 を共有、頂点 0 と 1 は独立
        assert!(shares_leaf(
            &m,
            DragTarget::PolyVertex { geom: 0, vert: 0 },
            DragTarget::PolyVertex { geom: 0, vert: 3 },
        ));
        assert!(!shares_leaf(
            &m,
            DragTarget::PolyVertex { geom: 0, vert: 0 },
            DragTarget::PolyVertex { geom: 0, vert: 1 },
        ));
    }

    #[test]
    fn links_pinned_axis_has_no_leaf() {
        let m = model_of(SRC_SEGMENTS);
        // 頂点 2 は x=x2 (var), y=y2 (let → 書き込み不可) なので葉は x2 の 1 つだけ
        assert_eq!(
            leaves_of(&m, DragTarget::PolyVertex { geom: 0, vert: 2 }).len(),
            1
        );
    }

    #[test]
    fn links_named_point_couples_segment_and_polygon() {
        let m = model_of(SRC_LINES);
        // v2 (geom 1) は l1 (geom 3) の終点と poly1 (geom 4) の頂点 1 に現れる
        let v2 = DragTarget::Point { geom: 1 };
        assert!(shares_leaf(
            &m,
            v2,
            DragTarget::SegmentEnd { geom: 3, end: 1 }
        ));
        assert!(shares_leaf(
            &m,
            v2,
            DragTarget::PolyVertex { geom: 4, vert: 1 }
        ));
        assert!(!shares_leaf(&m, v2, DragTarget::Point { geom: 0 }));
    }

    #[test]
    fn sketch_binding_names_lists_top_level_sketches() {
        let module = parse::parse(SRC_SEGMENTS).unwrap();
        assert_eq!(sketch_binding_names(&module), vec!["sk".to_string()]);
    }

    #[test]
    fn drag_vertex_writes_shared_var() {
        // 頂点 0 (x1, 0.0) を (1.0, 0.5) へ: x1 は var → 書き換え。
        // x1 は頂点 3 でも使われているので連動する。
        let out = drag(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 0 },
            DragValue::Pos([1.0, 0.5]),
        )
        .expect("drag");
        assert!(out.source.contains("var x1 = 1.0"), "{}", out.source);
        // 頂点 0 の y はインラインリテラル 0.0 → 0.5 に書き換え
        let SketchGeom::Polygon { verts, .. } = &out.model.geoms[0] else {
            panic!()
        };
        assert_eq!(verts[0], [1.0, 0.5]);
        assert_eq!(verts[3][0], 1.0, "x1 共有頂点も連動する");
        assert!(out.pinned.is_empty(), "{:?}", out.pinned);
    }

    #[test]
    fn drag_pinned_axis_is_reported() {
        // y2 は let → y 軸固定。x のみ適用される。
        let out = drag(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 2 },
            DragValue::Pos([5.0, 9.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var x2 = 5.0"), "{}", out.source);
        assert!(out.source.contains("let y2 = 3.0"), "{}", out.source);
        assert_eq!(out.pinned.len(), 1);
        assert!(out.pinned[0].contains("y"), "{:?}", out.pinned);
    }

    #[test]
    fn drag_all_pinned_is_rejected() {
        let src = "sk =\n    sketch\n        let x = 1.0\n        let y = 2.0\n        p = p2 x y\n    in\n    { p = p }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 4.0]),
        )
        .expect_err("should reject");
        assert!(matches!(err, DragReject::ReadOnly(_)), "{err:?}");
    }

    #[test]
    fn drag_ambiguous_is_rejected() {
        let src = "sk =\n    sketch\n        var a = 1.0\n        var b = 2.0\n        p = p2 (a + b) (a + b)\n    in\n    { p = p }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 4.0]),
        )
        .expect_err("should reject");
        assert!(matches!(err, DragReject::Ambiguous(_)), "{err:?}");
    }

    #[test]
    fn drag_offset_pattern_preserves_literal() {
        // `x1 + 1.0` は x1 に押し込まれ、オフセット 1.0 は保持される。
        let src = "sk =\n    sketch\n        var x1 = 2.0\n        p = p2 (x1 + 1.0) 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([5.0, 0.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var x1 = 4.0"), "{}", out.source);
        assert!(out.source.contains("x1 + 1.0"), "{}", out.source);
    }

    #[test]
    fn drag_through_let_pushes_into_var() {
        // let は導出値: t = b + 150.0 の t をドラッグすると b が書き換わる。
        let src = "sk =\n    sketch\n        var b = -40.0\n        let t = b + 150.0\n        p = p2 0.0 t\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([0.0, 120.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var b = -30.0"), "{}", out.source);
        assert!(out.source.contains("let t = b + 150.0"), "{}", out.source);
        let SketchGeom::Point { pos, .. } = &out.model.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [0.0, 120.0]);
    }

    #[test]
    fn drag_through_let_chain() {
        // let の多段: t2 = t * 2.0, t = b + 5.0 → b まで押し込む。
        let src = "sk =\n    sketch\n        var b = 10.0\n        let t = b + 5.0\n        let t2 = t * 2.0\n        p = p2 t2 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([40.0, 0.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var b = 15.0"), "{}", out.source);
    }

    #[test]
    fn drag_through_let_ambiguous_when_var_on_both_sides() {
        let src = "sk =\n    sketch\n        var b = 1.0\n        let t = b + 1.0\n        p = p2 (t + b) (t + b)\n    in\n    { p = p }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([9.0, 8.0]),
        )
        .expect_err("should reject");
        assert!(matches!(err, DragReject::Ambiguous(_)), "{err:?}");
    }

    #[test]
    fn drag_through_let_conflicts_with_direct_var_axis() {
        // x = t (→ b へ押し込み)、y = b。異なる値の書き込みは衝突。
        let src = "sk =\n    sketch\n        var b = 0.0\n        let t = b + 150.0\n        p = p2 t b\n    in\n    { p = p }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([160.0, 20.0]),
        )
        .expect_err("should conflict");
        assert!(matches!(err, DragReject::Conflict(_)), "{err:?}");
    }

    #[test]
    fn drag_through_foreign_let_chain() {
        // cross-sketch 参照 + let の多段: skB の座標から skA の var まで押し込む。
        let src = "skA =\n    sketch\n        var z = 5.0\n        let a = z * 2.0\n        let b = a + 1.0\n        p = p2 z 0.0\n    in\n    { p = p, b }\n    end\n\nskB =\n    sketch\n        let c = skA.b + 1.0\n        p = p2 c 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "skB",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([20.0, 0.0]),
        )
        .expect("drag");
        // c = ((z * 2) + 1) + 1 なので c = 20 → z = 9
        assert!(out.source.contains("var z = 9.0"), "{}", out.source);
    }

    #[test]
    fn model_resolves_foreign_let_and_literal_let() {
        // 他 sketch の export (計算 let / リテラル let) を座標に使える。宣言順も不問。
        let src = "skB =\n    sketch\n        q = p2 skA.w skA.zc\n    in\n    { q = q }\n    end\n\nskA =\n    sketch\n        var z1 = 50.0\n        let zc = z1 + 10.0\n        let w = 10.0\n        p = p2 0.0 z1\n    in\n    { p = p, zc, w }\n    end\n";
        let m = model_from_source(src, "skB").expect("model");
        let SketchGeom::Point { pos, .. } = &m.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [10.0, 60.0]);
    }

    #[test]
    fn drag_foreign_literal_let_axis_is_readonly() {
        // リテラルのみの let export は凍結値: 書き込めず pinned になる。
        // 計算 let export は RHS の var へ押し込まれる。
        let src = "skA =\n    sketch\n        var z1 = 50.0\n        let zc = z1 + 10.0\n        let w = 10.0\n        p = p2 0.0 z1\n    in\n    { p = p, zc, w }\n    end\n\nskB =\n    sketch\n        q = p2 skA.w skA.zc\n    in\n    { q = q }\n    end\n";
        let out = drag(
            src,
            "skB",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([99.0, 70.0]),
        )
        .expect("drag");
        assert!(out.source.contains("let w = 10.0"), "{}", out.source);
        assert!(out.source.contains("var z1 = 60.0"), "{}", out.source);
        assert_eq!(out.pinned.len(), 1, "{:?}", out.pinned);
        assert!(out.pinned[0].contains("x"), "{:?}", out.pinned);
    }

    #[test]
    fn drag_foreign_let_resolves_ambiguity_to_var() {
        // `let l = c - (v - c)` の c がリテラル let export なら両辺 writable に
        // ならず、v への一意な書き込みに解決される。
        let src = "skC =\n    sketch\n        let c = 60.0\n        pc = p2 c 0.0\n    in\n    { pc = pc, c }\n    end\n\nsk =\n    sketch\n        var v = 102.0\n        let l = skC.c - (v - skC.c)\n        p = p2 l 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([30.0, 0.0]),
        )
        .expect("drag");
        // l = 2c - v なので l = 30 → v = 90
        assert!(out.source.contains("var v = 90.0"), "{}", out.source);
        assert!(out.source.contains("let c = 60.0"), "{}", out.source);
    }

    #[test]
    fn foreign_point_axis_reads_and_drags() {
        // export された点の座標を `A.pt.x` / `.y` で参照でき、ドラッグは
        // export 元の座標リテラルへ書き込まれる (両 sketch が連動)。
        let src = "skA =\n    sketch\n        anchor = p2 5.0 40.0\n    in\n    { anchor }\n    end\n\nskB =\n    sketch\n        p = p2 skA.anchor.x 0.0\n        q = p2 0.0 skA.anchor.y\n    in\n    { p = p, q = q }\n    end\n";
        let m = model_from_source(src, "skB").expect("model");
        let SketchGeom::Point { pos, .. } = &m.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [5.0, 0.0]);
        let SketchGeom::Point { pos, .. } = &m.geoms[1] else {
            panic!()
        };
        assert_eq!(*pos, [0.0, 40.0]);

        let out = drag(
            src,
            "skB",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([7.0, 1.0]),
        )
        .expect("drag");
        assert!(
            out.source.contains("anchor = p2 7.0 40.0"),
            "{}",
            out.source
        );
        let ma = model_from_source(&out.source, "skA").expect("model");
        let SketchGeom::Point { pos, .. } = &ma.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [7.0, 40.0], "export 元の点も連動する");
    }

    #[test]
    fn foreign_point_axis_pushes_into_var() {
        // 点座標が var 参照なら var へ押し込む。
        let src = "skA =\n    sketch\n        var ax = 5.0\n        anchor = p2 ax 40.0\n    in\n    { anchor }\n    end\n\nskB =\n    sketch\n        p = p2 skA.anchor.x 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "skB",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([7.0, 0.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var ax = 7.0"), "{}", out.source);
    }

    #[test]
    fn cross_sketch_cycle_is_rejected() {
        let src = "skA =\n    sketch\n        let a = skB.b + 1.0\n        p = p2 a 0.0\n    in\n    { p = p, a }\n    end\n\nskB =\n    sketch\n        let b = skA.a + 1.0\n        q = p2 b 0.0\n    in\n    { q = q, b }\n    end\n";
        let err = model_from_source(src, "skA").expect_err("cycle");
        assert!(err.contains("循環"), "{err}");
        let err = model_from_source(src, "skB").expect_err("cycle");
        assert!(err.contains("循環"), "{err}");
    }

    #[test]
    fn self_reference_is_rejected() {
        let src = "skA =\n    sketch\n        var v = 1.0\n        let a = skA.v + 1.0\n        p = p2 a 0.0\n    in\n    { p = p, v }\n    end\n";
        let err = model_from_source(src, "skA").expect_err("self ref");
        assert!(err.contains("循環"), "{err}");
    }

    #[test]
    fn remove_geom_rejects_export_referenced_from_other_sketch() {
        let src = "skA =\n    sketch\n        anchor = p2 5.0 40.0\n        base = p2 0.0 0.0\n    in\n    { anchor, base }\n    end\n\nskB =\n    sketch\n        p = p2 skA.anchor.x 0.0\n    in\n    { p = p }\n    end\n";
        let err = remove_geom(src, "skA", "anchor").expect_err("reject");
        assert!(err.contains("skB"), "{err}");
        assert!(err.contains("skA.anchor"), "{err}");
        // 参照されていない export は削除できる
        let s = remove_geom(src, "skA", "base").expect("remove");
        assert!(!s.contains("base"), "{s}");
    }

    #[test]
    fn remove_geom_rejects_export_referenced_from_functional_layer() {
        let src = "main = skA.anchor.x\n\nskA =\n    sketch\n        anchor = p2 5.0 40.0\n    in\n    { anchor }\n    end\n";
        let err = remove_geom(src, "skA", "anchor").expect_err("reject");
        assert!(err.contains("main"), "{err}");
    }

    #[test]
    fn field_ref_rejects_locally_shadowed_receiver() {
        // ローカル束縛より後の参照は shadow され、sketch 参照として使えない
        // (sema / runtime の逐次スコープと一致)。
        let src = "other =\n    sketch\n        var w = 5.0\n        o = p2 w 0.0\n    in\n    { o = o, w }\n    end\n\nsk =\n    sketch\n        let other = 1.0\n        p = p2 other.w 0.0\n    in\n    { p = p }\n    end\n";
        let err = model_from_source(src, "sk").expect_err("reject");
        assert!(err.contains("ブロック内の束縛"), "{err}");
    }

    #[test]
    fn field_ref_before_local_binding_resolves_to_sketch() {
        // ローカル束縛より前の参照は (runtime と同じく) sketch へ解決される。
        let src = "other =\n    sketch\n        var w = 5.0\n        o = p2 w 0.0\n    in\n    { o = o, w }\n    end\n\nsk =\n    sketch\n        p = p2 other.w 0.0\n        let other = 1.0\n    in\n    { p = p }\n    end\n";
        let m = model_from_source(src, "sk").expect("model");
        let SketchGeom::Point { pos, .. } = &m.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [5.0, 0.0]);
    }

    #[test]
    fn non_finite_scalar_is_rejected() {
        // リテラル積のオーバーフロー (Inf) は評価エラーになり座標へ流れない。
        let big = "999999999999999999999999999999999999999.0"; // ~1e39
        let src = format!(
            "sk =\n    sketch\n        let b1 = {big} * {big}\n        let b2 = b1 * b1\n        let b3 = b2 * b2\n        p = p2 b3 0.0\n    in\n    {{ p = p }}\n    end\n"
        );
        let err = model_from_source(&src, "sk").expect_err("reject");
        assert!(err.contains("有限値になりません"), "{err}");
    }

    #[test]
    fn duplicated_sketch_names_are_rejected() {
        let src = "sk =\n    sketch\n        p = p2 1.0 1.0\n    in\n    { p = p }\n    end\n\nsk =\n    sketch\n        q = p2 2.0 2.0\n    in\n    { q = q }\n    end\n";
        let err = model_from_source(src, "sk").expect_err("reject");
        assert!(err.contains("同名"), "{err}");
        let err = add_point(src, "sk", [0.0, 0.0])
            .map(|_| ())
            .expect_err("reject");
        assert!(err.contains("同名"), "{err}");
    }

    #[test]
    fn cycle_member_and_dependent_get_distinct_errors() {
        let src = "skZ =\n    sketch\n        p = p2 skA.a 0.0\n    in\n    { p = p }\n    end\n\nskA =\n    sketch\n        let a = skB.b + 1.0\n        q = p2 a 0.0\n    in\n    { q = q, a }\n    end\n\nskB =\n    sketch\n        let b = skA.a + 1.0\n        r = p2 b 0.0\n    in\n    { r = r, b }\n    end\n";
        let err = model_from_source(src, "skA").expect_err("cycle member");
        assert!(err.contains("参照循環に含まれています"), "{err}");
        let err = model_from_source(src, "skZ").expect_err("dependent");
        assert!(err.contains("依存しているため"), "{err}");
        assert!(err.contains("skA"), "{err}");
    }

    #[test]
    fn add_point_avoids_sketch_binding_names() {
        // 生成名 pt1 が top-level sketch 名と衝突する場合は pt2 になる。
        let src = "pt1 =\n    sketch\n        o = p2 0.0 0.0\n    in\n    { o = o }\n    end\n\nsk =\n    sketch\n        q = p2 1.0 1.0\n    in\n    { q = q }\n    end\n";
        let (s, name) = add_point(src, "sk", [3.0, 2.0]).expect("add");
        assert_eq!(name, "pt2");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn unrelated_broken_sketch_does_not_block_target() {
        // 参照していない sketch の評価エラーや循環は対象の操作に影響しない。
        let src = "skBroken =\n    sketch\n        let x = nope + 1.0\n        p = p2 x 0.0\n    in\n    { p = p }\n    end\n\nskOk =\n    sketch\n        q = p2 1.0 2.0\n    in\n    { q = q }\n    end\n";
        let m = model_from_source(src, "skOk").expect("model");
        assert_eq!(m.geoms.len(), 1);
        drag(
            src,
            "skOk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 4.0]),
        )
        .expect("drag");
    }

    #[test]
    fn inspect_foreign_var_reports_qualified_name() {
        let src = "skA =\n    sketch\n        var z1 = 50.0\n        p = p2 0.0 z1\n    in\n    { p = p, z1 }\n    end\n\nskB =\n    sketch\n        q = p2 3.0 skA.z1\n    in\n    { q = q }\n    end\n";
        let info = inspect(src, "skB", DragTarget::Point { geom: 0 }).expect("inspect");
        assert_eq!(info.axes[1].writes[0].name.as_deref(), Some("skA.z1"));
        // export 元 sketch では修飾なし
        let info = inspect(src, "skA", DragTarget::Point { geom: 0 }).expect("inspect");
        assert_eq!(info.axes[1].writes[0].name.as_deref(), Some("z1"));
    }

    #[test]
    fn drag_division_inverts_through_numerator() {
        let src = "sk =\n    sketch\n        var w = 8.0\n        p = p2 (w / 2.0) 0.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 0.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var w = 6.0"), "{}", out.source);
    }

    #[test]
    fn drag_negative_literal_stays_parenthesized() {
        let src = "sk =\n    sketch\n        p = p2 (-2.0) 3.0\n    in\n    { p = p }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([-5.0, -1.0]),
        )
        .expect("drag");
        assert!(out.source.contains("p2 (-5.0) (-1.0)"), "{}", out.source);
        // 再パースできる
        model_from_source(&out.source, "sk").expect("reparse");
    }

    #[test]
    fn drag_conflict_same_var_both_axes() {
        let src = "sk =\n    sketch\n        var a = 1.0\n        p = p2 a a\n    in\n    { p = p }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 4.0]),
        )
        .expect_err("should conflict");
        assert!(matches!(err, DragReject::Conflict(_)), "{err:?}");
    }

    #[test]
    fn drag_junction_updates_named_point_once() {
        // line 形式 polygon の junction は名前付き点 1 箇所の書き込みになる。
        let out = drag(
            SRC_LINES,
            "sk",
            DragTarget::PolyVertex { geom: 4, vert: 1 },
            DragValue::Pos([5.0, 1.0]),
        )
        .expect("drag");
        assert!(out.source.contains("v2 = p2 5.0 1.0"), "{}", out.source);
        let SketchGeom::Polygon { verts, .. } = &out.model.geoms[4] else {
            panic!()
        };
        assert_eq!(verts[1], [5.0, 1.0]);
    }

    #[test]
    fn drag_junction_inline_duplicate_literals_both_updated() {
        let src = "sk =\n    sketch\n        poly1 = polygon [line (p2 0.0 0.0) (p2 4.0 0.0), line (p2 4.0 0.0) (p2 0.0 3.0), line (p2 0.0 3.0) (p2 0.0 0.0)]\n    in\n    { poly1 = poly1 }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 1 },
            DragValue::Pos([5.0, 1.0]),
        )
        .expect("drag");
        // junction (4.0, 0.0) の 2 出現が両方 (5.0, 1.0) になる
        assert_eq!(
            out.source.matches("p2 5.0 1.0").count(),
            2,
            "{}",
            out.source
        );
        let SketchGeom::Polygon { verts, .. } = &out.model.geoms[0] else {
            panic!()
        };
        assert_eq!(verts[1], [5.0, 1.0]);
    }

    #[test]
    fn drag_circle_center_and_radius() {
        let src = "sk =\n    sketch\n        circ1 = circle 2.0 |> translate2d (p2 0.0 0.0) (p2 4.0 5.0)\n    in\n    { circ1 = circ1 }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::CircleCenter { geom: 0 },
            DragValue::Pos([7.0, 8.0]),
        )
        .expect("drag center");
        assert!(out.source.contains("p2 7.0 8.0"), "{}", out.source);
        let out2 = drag(
            &out.source,
            "sk",
            DragTarget::CircleRadius { geom: 0 },
            DragValue::Radius(3.5),
        )
        .expect("drag radius");
        assert!(out2.source.contains("circle 3.5"), "{}", out2.source);
    }

    #[test]
    fn drag_origin_circle_center_rejected() {
        let src = "sk =\n    sketch\n        circ1 = circle 2.0\n    in\n    { circ1 = circ1 }\n    end\n";
        let err = drag(
            src,
            "sk",
            DragTarget::CircleCenter { geom: 0 },
            DragValue::Pos([1.0, 1.0]),
        )
        .expect_err("should reject");
        assert!(matches!(err, DragReject::ReadOnly(_)), "{err:?}");
    }

    #[test]
    fn empty_sketch_add_and_remove_last_roundtrip() {
        let src = "sk =\n    sketch\n    in\n    {}\n    end\n";
        assert_eq!(model_from_source(src, "sk").expect("model").geoms.len(), 0);

        let (s1, name) = add_point(src, "sk", [3.0, 2.0]).expect("add_point");
        assert!(
            s1.contains("sketch\n        pt1 = p2 3.0 2.0\n    in"),
            "{s1}"
        );
        assert!(s1.contains("{ pt1 }"), "{s1}");
        assert_eq!(model_from_source(&s1, "sk").expect("model").geoms.len(), 1);

        let s2 = remove_geom(&s1, "sk", &name).expect("remove last geom");
        assert!(s2.contains("{}"), "{s2}");
        assert_eq!(model_from_source(&s2, "sk").expect("model").geoms.len(), 0);
    }

    #[test]
    fn add_point_and_remove_geom_roundtrip() {
        let (s1, _) = add_point(SRC_SEGMENTS, "sk", [3.0, -2.0]).expect("add_point");
        assert!(s1.contains("pt1 = p2 3.0 (-2.0)"), "{s1}");
        assert!(s1.contains("{ poly1 = poly1, pt1 }"), "{s1}");
        let m = model_from_source(&s1, "sk").expect("model");
        assert_eq!(m.geoms.len(), 2);

        let s2 = remove_geom(&s1, "sk", "pt1").expect("remove");
        assert!(!s2.contains("pt1"), "{s2}");
        assert!(s2.contains("{ poly1 = poly1 }"), "{s2}");
    }

    #[test]
    fn add_circle_and_polygon() {
        let (s1, _) = add_circle(SRC_SEGMENTS, "sk", [2.0, 3.0], 1.5).expect("add_circle");
        assert!(
            s1.contains("circ1 = circle 1.5 |> translate2d (p2 0.0 0.0) (p2 2.0 3.0)"),
            "{s1}"
        );
        let (s2, poly_name) =
            add_polygon(&s1, "sk", &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]).expect("add_polygon");
        assert_eq!(poly_name, "poly2");
        assert!(
            s2.contains("poly2 = polygon (segments [p2 0.0 0.0, p2 1.0 0.0, p2 0.0 1.0])"),
            "{s2}"
        );
        let m = model_from_source(&s2, "sk").expect("model");
        assert_eq!(m.geoms.len(), 3);
    }

    #[test]
    fn append_vertex_to_segments_polygon() {
        let s = append_polygon_vertex(SRC_SEGMENTS, "sk", "poly1", [9.0, 9.0]).expect("append");
        assert!(s.contains("p2 x1 y2, p2 9.0 9.0])"), "{s}");
        let m = model_from_source(&s, "sk").expect("model");
        let SketchGeom::Polygon { verts, .. } = &m.geoms[0] else {
            panic!()
        };
        assert_eq!(verts.len(), 5);
    }

    #[test]
    fn append_vertex_to_line_polygon_rejected() {
        let err = append_polygon_vertex(SRC_LINES, "sk", "poly1", [9.0, 9.0]).expect_err("reject");
        assert!(err.contains("segments"), "{err}");
    }

    #[test]
    fn remove_referenced_point_rejected() {
        let err = remove_geom(SRC_LINES, "sk", "v1").expect_err("reject");
        assert!(err.contains("参照されている"), "{err}");
    }

    #[test]
    fn remove_middle_record_field() {
        let src = "sk =\n    sketch\n        pt1 = p2 0.0 0.0\n        pt2 = p2 1.0 0.0\n        pt3 = p2 2.0 0.0\n    in\n    { pt1 = pt1, pt2 = pt2, pt3 = pt3 }\n    end\n";
        let s = remove_geom(src, "sk", "pt2").expect("remove");
        assert!(s.contains("{ pt1 = pt1, pt3 = pt3 }"), "{s}");
        assert!(!s.contains("pt2"), "{s}");
    }

    #[test]
    fn remove_last_field_makes_empty_record() {
        let src = "sk =\n    sketch\n        pt1 = p2 0.0 0.0\n        pt2 = p2 1.0 0.0\n    in\n    { pt2 = pt2 }\n    end\n";
        let s = remove_geom(src, "sk", "pt2").expect("remove");
        assert!(s.contains("{}"), "{s}");
    }

    #[test]
    fn drag_after_structural_edit_roundtrip() {
        // 追加した点をドラッグ → モデルに反映される
        let (s1, _) = add_point(SRC_SEGMENTS, "sk", [1.0, 1.0]).expect("add");
        let out = drag(
            &s1,
            "sk",
            DragTarget::Point { geom: 1 },
            DragValue::Pos([6.0, -7.5]),
        )
        .expect("drag");
        let SketchGeom::Point { pos, .. } = &out.model.geoms[1] else {
            panic!()
        };
        assert_eq!(*pos, [6.0, -7.5]);
    }

    #[test]
    fn add_point_negative_zero_is_normalized() {
        // キャンバスのスナップ計算は -0.0 を作ることがある
        let (s, _) = add_point(SRC_SEGMENTS, "sk", [11.0, -0.0]).expect("add");
        assert!(s.contains("p2 11.0 0.0"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn drag_to_negative_zero_writes_plain_zero() {
        let src =
            "sk =\n    sketch\n        pt1 = p2 1.0 2.0\n    in\n    { pt1 = pt1 }\n    end\n";
        let out = drag(
            src,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([11.0, -0.0]),
        )
        .expect("drag");
        assert!(out.source.contains("p2 11.0 0.0"), "{}", out.source);
    }

    #[test]
    fn factor_vars_groups_per_axis() {
        let src = "sk =\n    sketch\n        poly1 = polygon (segments [p2 0.0 0.0, p2 4.0 0.0, p2 4.0 3.0, p2 0.0 3.0])\n    in\n    { poly1 = poly1 }\n    end\n";
        let before = model_of(src);
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x1 = 0.0"), "{s}");
        assert!(s.contains("var x2 = 4.0"), "{s}");
        // 0.0 は y 軸にも 2 回現れるが、x1 とは別 var になる
        assert!(s.contains("var y1 = 0.0"), "{s}");
        assert!(s.contains("var y2 = 3.0"), "{s}");
        assert!(
            s.contains("segments [p2 x1 y1, p2 x2 y1, p2 x2 y2, p2 x1 y2]"),
            "{s}"
        );
        assert_eq!(model_of(&s).geoms, before.geoms, "形状は変わらない");
    }

    #[test]
    fn factor_vars_nothing_to_merge_is_rejected() {
        // x の 2.0 は var 参照とリテラルで 1 回ずつ → リテラル同士でないのでまとめない
        let src = "sk =\n    sketch\n        var x1 = 2.0\n        pt1 = p2 x1 1.0\n        pt2 = p2 2.0 5.0\n    in\n    { pt1 = pt1, pt2 = pt2 }\n    end\n";
        let err = factor_vars(src, "sk").expect_err("reject");
        assert!(err.contains("まとめられる"), "{err}");
    }

    #[test]
    fn factor_vars_avoids_existing_names() {
        let src = "sk =\n    sketch\n        var x1 = 9.0\n        pt1 = p2 x1 0.0\n        pt2 = p2 5.0 0.0\n        pt3 = p2 5.0 1.0\n    in\n    { pt1 = pt1, pt2 = pt2, pt3 = pt3 }\n    end\n";
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x2 = 5.0"), "{s}");
        assert!(s.contains("p2 x2 0.0") || s.contains("p2 x2 y1"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn factor_vars_shares_junction_literals_once() {
        // line 形式 polygon の junction (同一 span) は 1 回として数える。
        // x 軸は 4.0 のみ 2 回 (v2, v3)、y 軸は 0.0 のみ 2 回 (v1, v2)。
        let s = factor_vars(SRC_LINES, "sk").expect("factor");
        assert!(s.contains("var x1 = 4.0"), "{s}");
        assert!(s.contains("var y1 = 0.0"), "{s}");
        assert!(s.contains("v1 = p2 0.0 y1"), "{s}");
        assert!(s.contains("v2 = p2 x1 y1"), "{s}");
        assert!(s.contains("v3 = p2 x1 3.0"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn factor_vars_skips_circle_src_and_radius() {
        // translate2d の src (p2 0.0 0.0) と半径 3.0 は対象外
        let src = "sk =\n    sketch\n        pt1 = p2 0.0 3.0\n        circ1 = circle 3.0 |> translate2d (p2 0.0 0.0) (p2 0.0 6.0)\n    in\n    { pt1 = pt1, circ1 = circ1 }\n    end\n";
        // x: pt1 の 0.0 と dst の 0.0 の 2 回 → まとめる。src の 0.0 は数えない
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x1 = 0.0"), "{s}");
        assert!(s.contains("pt1 = p2 x1 3.0"), "{s}");
        assert!(s.contains("circle 3.0"), "半径はまとめない: {s}");
        assert!(s.contains("translate2d (p2 0.0 0.0) (p2 x1 6.0)"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn factor_vars_negative_literals() {
        let src = "sk =\n    sketch\n        pt1 = p2 (-1.5) 0.0\n        pt2 = p2 (-1.5) 2.0\n    in\n    { pt1 = pt1, pt2 = pt2 }\n    end\n";
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x1 = -1.5"), "{s}");
        let m = model_from_source(&s, "sk").expect("parses");
        assert_eq!(m.geoms.len(), 2);
    }

    const SRC_CROSS: &str = "skxz =\n    sketch\n        var z1 = 50.0\n        var x1 = 10.0\n        p = p2 x1 z1\n    in\n    { p = p, z1 }\n    end\n\nskyz =\n    sketch\n        q = p2 3.0 skxz.z1\n        r = p2 4.0 (skxz.z1 + 10.0)\n    in\n    { q = q, r = r }\n    end\n";

    #[test]
    fn model_reads_foreign_var() {
        let m = model_from_source(SRC_CROSS, "skxz").expect("model");
        let SketchGeom::Point { pos, .. } = &m.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [10.0, 50.0]);
        let m2 = model_from_source(SRC_CROSS, "skyz").expect("model");
        let SketchGeom::Point { pos, .. } = &m2.geoms[1] else {
            panic!()
        };
        assert_eq!(*pos, [4.0, 60.0]);
    }

    #[test]
    fn drag_writes_foreign_var_and_export_sketch_follows() {
        let out = drag(
            SRC_CROSS,
            "skyz",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([3.0, 70.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var z1 = 70.0"), "{}", out.source);
        // export 元の sketch も新ソースでは連動する
        let m2 = model_from_source(&out.source, "skxz").expect("model");
        let SketchGeom::Point { pos, .. } = &m2.geoms[0] else {
            panic!()
        };
        assert_eq!(*pos, [10.0, 70.0]);
    }

    #[test]
    fn drag_offset_expr_pushes_into_foreign_var() {
        // r の y は `skxz.z1 + 10.0` → z1 に押し込まれ、オフセットは保持される。
        let out = drag(
            SRC_CROSS,
            "skyz",
            DragTarget::Point { geom: 1 },
            DragValue::Pos([4.0, 90.0]),
        )
        .expect("drag");
        assert!(out.source.contains("var z1 = 80.0"), "{}", out.source);
        assert!(out.source.contains("skxz.z1 + 10.0"), "{}", out.source);
    }

    #[test]
    fn factor_vars_avoids_sketch_binding_names() {
        // 生成名が top-level sketch 名と衝突しない (衝突すると `x1.field` 参照が壊れる)。
        let src = "x1 =\n    sketch\n        pt0 = p2 0.0 9.0\n    in\n    { pt0 = pt0 }\n    end\n\nsk =\n    sketch\n        pt1 = p2 5.0 0.0\n        pt2 = p2 5.0 1.0\n    in\n    { pt1 = pt1, pt2 = pt2 }\n    end\n";
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x2 = 5.0"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn factor_vars_ignores_foreign_exports() {
        // 他 sketch の export と同値のリテラルがあっても参照に置き換えたりしない。
        let src = "skA =\n    sketch\n        var w = 4.0\n        pw = p2 w 0.0\n    in\n    { pw = pw, w }\n    end\n\nsk =\n    sketch\n        pt1 = p2 4.0 1.0\n        pt2 = p2 4.0 2.0\n    in\n    { pt1 = pt1, pt2 = pt2 }\n    end\n";
        let s = factor_vars(src, "sk").expect("factor");
        assert!(s.contains("var x1 = 4.0"), "{s}");
        assert!(s.contains("pt1 = p2 x1 1.0"), "{s}");
        model_from_source(&s, "sk").expect("parses");
    }

    #[test]
    fn writeback_ambiguity_reported_across_sketches() {
        // 座標式の両辺が writable (foreign var + local var) なら曖昧として報告する。
        let src = "skA =\n    sketch\n        var z1 = 50.0\n        p = p2 z1 0.0\n    in\n    { p = p, z1 }\n    end\n\nskB =\n    sketch\n        var w = 1.0\n        q = p2 (skA.z1 + w) 0.0\n    in\n    { q = q }\n    end\n";
        let module = parse::parse(src).unwrap();
        let warns = writeback_ambiguities(&module);
        assert_eq!(warns.len(), 1, "{warns:?}");
        assert!(warns[0].message.contains("skB"), "{warns:?}");
        assert!(warns[0].message.contains("曖昧"), "{warns:?}");
    }

    #[test]
    fn writeback_ambiguities_reports_two_var_binop() {
        // (a + b) は両辺に var → 曖昧。y 軸のリテラルと var x1 は報告されない。
        // 読み取り専用 (let のみ) の軸も報告されない。
        let src = "sk =\n    sketch\n        var a = 1.0\n        var b = 2.0\n        var x1 = 3.0\n        let frozen = 4.0\n        p = p2 (a + b) 0.0\n        q = p2 x1 frozen\n    in\n    { p = p, q = q }\n    end\n";
        let module = parse::parse(src).unwrap();
        let warns = writeback_ambiguities(&module);
        assert_eq!(warns.len(), 1, "{warns:?}");
        assert!(warns[0].message.contains("sk"), "{warns:?}");
        assert!(warns[0].message.contains("p の x"), "{warns:?}");
        assert!(warns[0].message.contains("曖昧"), "{warns:?}");
        // span は `(a + b)` の中身を指す
        assert_eq!(&src[warns[0].span.range()], "a + b");
    }

    #[test]
    fn writeback_ambiguities_dedupes_junction() {
        // 名前付き点が line と polygon から共有されても報告は 1 回。
        let src = "sk =\n    sketch\n        var a = 1.0\n        var b = 2.0\n        v1 = p2 (a + b) 0.0\n        v2 = p2 4.0 0.0\n        v3 = p2 4.0 3.0\n        poly1 = polygon [line v1 v2, line v2 v3, line v3 v1]\n    in\n    { poly1 = poly1 }\n    end\n";
        let module = parse::parse(src).unwrap();
        let warns = writeback_ambiguities(&module);
        assert_eq!(warns.len(), 1, "{warns:?}");
    }

    #[test]
    fn inspect_vertex_reports_var_and_literal() {
        let info = inspect(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 0 },
        )
        .expect("inspect");
        assert_eq!(info.axes.len(), 2);
        // x = x1 (var)
        assert_eq!(info.axes[0].label, "x");
        assert_eq!(info.axes[0].value, 0.0);
        assert_eq!(info.axes[0].writes.len(), 1);
        assert_eq!(info.axes[0].writes[0].name.as_deref(), Some("x1"));
        assert_eq!(info.axes[0].writes[0].value, 0.0);
        // y = 0.0 (匿名リテラル)
        assert_eq!(info.axes[1].writes[0].name, None);
        assert_eq!(info.axes[1].writes[0].value, 0.0);
        assert!(info.axes[1].readonly.is_none());
    }

    #[test]
    fn inspect_let_axis_is_readonly() {
        let info = inspect(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 2 },
        )
        .expect("inspect");
        assert_eq!(info.axes[0].writes[0].name.as_deref(), Some("x2"));
        assert!(info.axes[1].writes.is_empty());
        assert!(info.axes[1].readonly.is_some());
    }

    #[test]
    fn inspect_offset_expr_reports_source_var_value() {
        // 座標は x1 + 1.0 = 3.0 だが、書き込み先 var の現在値は x1 = 2.0。
        let src = "sk =\n    sketch\n        var x1 = 2.0\n        p = p2 (x1 + 1.0) 0.0\n    in\n    { p = p }\n    end\n";
        let info = inspect(src, "sk", DragTarget::Point { geom: 0 }).expect("inspect");
        assert_eq!(info.axes[0].value, 3.0);
        assert_eq!(info.axes[0].writes[0].name.as_deref(), Some("x1"));
        assert_eq!(info.axes[0].writes[0].value, 2.0);
    }

    #[test]
    fn inspect_circle_center_and_radius() {
        let src = "sk =\n    sketch\n        var r = 2.0\n        circ1 = circle r |> translate2d (p2 1.0 1.0) (p2 4.0 6.0)\n    in\n    { circ1 = circ1 }\n    end\n";
        let info = inspect(src, "sk", DragTarget::CircleCenter { geom: 0 }).expect("inspect");
        assert_eq!(info.axes[0].value, 3.0, "中心座標 = dst - src");
        assert_eq!(
            info.axes[0].writes[0].value, 4.0,
            "書き込み先は dst のリテラル"
        );
        let ri = inspect(src, "sk", DragTarget::CircleRadius { geom: 0 }).expect("inspect");
        assert_eq!(ri.axes.len(), 1);
        assert_eq!(ri.axes[0].label, "半径");
        assert_eq!(ri.axes[0].writes[0].name.as_deref(), Some("r"));
    }

    #[test]
    fn inspect_origin_circle_center_is_readonly() {
        let src = "sk =\n    sketch\n        circ1 = circle 2.0\n    in\n    { circ1 = circ1 }\n    end\n";
        let info = inspect(src, "sk", DragTarget::CircleCenter { geom: 0 }).expect("inspect");
        assert!(info.axes.iter().all(|a| a.readonly.is_some()));
    }

    #[test]
    fn inspect_junction_dedupes_shared_leaf() {
        // 名前付き点の junction は同じ葉に集約されるので write は 1 つ。
        let info =
            inspect(SRC_LINES, "sk", DragTarget::PolyVertex { geom: 4, vert: 1 }).expect("inspect");
        assert_eq!(info.axes[0].writes.len(), 1);
        assert_eq!(info.axes[0].writes[0].value, 4.0);
    }

    #[test]
    fn set_var_value_writes_var_and_linked_vertex_follows() {
        let out = set_var_value(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 0 },
            0,
            0,
            7.0,
        )
        .expect("set");
        assert!(out.source.contains("var x1 = 7.0"), "{}", out.source);
        let SketchGeom::Polygon { verts, .. } = &out.model.geoms[0] else {
            panic!()
        };
        assert_eq!(verts[0][0], 7.0);
        assert_eq!(verts[3][0], 7.0, "x1 共有頂点も連動する");
    }

    #[test]
    fn set_var_value_offset_expr_writes_var_directly() {
        // var の値そのものを書くので、座標は x1 + 1.0 = 6.0 になる。
        let src = "sk =\n    sketch\n        var x1 = 2.0\n        p = p2 (x1 + 1.0) 0.0\n    in\n    { p = p }\n    end\n";
        let out = set_var_value(src, "sk", DragTarget::Point { geom: 0 }, 0, 0, 5.0).expect("set");
        assert!(out.source.contains("var x1 = 5.0"), "{}", out.source);
        let SketchGeom::Point { pos, .. } = &out.model.geoms[0] else {
            panic!()
        };
        assert_eq!(pos[0], 6.0);
    }

    #[test]
    fn set_var_value_readonly_axis_rejected() {
        let err = set_var_value(
            SRC_SEGMENTS,
            "sk",
            DragTarget::PolyVertex { geom: 0, vert: 2 },
            1,
            0,
            9.0,
        )
        .expect_err("reject");
        assert!(
            err.contains("書き込める var がありません") || err.contains("範囲外"),
            "{err}"
        );
    }

    #[test]
    fn set_var_value_negative_literal_reparses() {
        let src =
            "sk =\n    sketch\n        pt1 = p2 1.0 2.0\n    in\n    { pt1 = pt1 }\n    end\n";
        let out = set_var_value(src, "sk", DragTarget::Point { geom: 0 }, 0, 0, -3.5).expect("set");
        assert!(out.source.contains("p2 (-3.5) 2.0"), "{}", out.source);
    }

    #[test]
    fn factor_vars_then_drag_moves_shared() {
        let src = "sk =\n    sketch\n        pt1 = p2 4.0 0.0\n        pt2 = p2 4.0 2.0\n    in\n    { pt1 = pt1, pt2 = pt2 }\n    end\n";
        let s = factor_vars(src, "sk").expect("factor");
        let out = drag(
            &s,
            "sk",
            DragTarget::Point { geom: 0 },
            DragValue::Pos([7.0, 0.0]),
        )
        .expect("drag");
        let SketchGeom::Point { pos, .. } = &out.model.geoms[1] else {
            panic!()
        };
        assert_eq!(pos[0], 7.0, "共有 var 経由で pt2 も連動する");
    }
}
