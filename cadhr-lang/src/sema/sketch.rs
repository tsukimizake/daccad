//! sketch DSL (`sketch .. in .. end`) の制約検査。
//!
//! sketch ブロックは GUI の 2D sketch workspace と双方向に紐付くため、
//! 逆評価 (頂点ドラッグ → コード書き戻し) が常に可能な形に式を制限する:
//!
//! - `var x = <符号付き Float リテラル>` — 逆評価の書き込み対象
//! - `let y = <スカラー式>` — 導出スカラー (リテラル / 既出スカラー名 / 四則演算 /
//!   sqrt sin cos tan)。逆評価は RHS を辿って var へ押し込む
//! - `name = <幾何式>` — `p2` / `line` / `polygon` / `circle` の束縛
//! - `/` の右辺は 0 でない Float リテラルのみ (逆評価が常に定義されるように)
//! - ブロック外の変数参照・Int リテラル・if / case / lambda 等は禁止。
//!   例外は同一モジュールの top-level sketch の export への参照 (下記)
//! - body は `{ f = 束縛名, ... }` の record か単一の幾何名。record の field には
//!   幾何だけでなく var / let (スカラー) も書ける
//! - polygon の線分列は静的に連結 + 閉路であること
//!
//! ## sketch 間の座標共有 (export / cross-sketch 参照)
//!
//! body record に入れた束縛は他 sketch のスカラー式から参照できる:
//!
//! - `<sketch名>.<field名>` — export されたスカラー (var / let)
//! - `<sketch名>.<点field>.x` / `.y` — export された点の座標
//! - 参照できるのは同一モジュールの top-level sketch binding のみ。自己参照は不可
//! - sketch 単位の参照循環はエラー (runtime の依存順評価も循環を拒否する)
//!
//! 検査は構文的 (型推論より前に実行できる)。座標値はブロック内で静的に決まるので
//! 連結性検査のためにここで評価する。sketch は参照の依存順 (topo 順) に検査し、
//! export された静的値を後続の sketch へ引き渡す。

use crate::diagnostic::{Diagnostic, Span};
use crate::geom::{MATH_FNS, P2, Seg2, eval_math_fn, p2};
use crate::sketch::{TopSketch, export_map, sketch_topo, top_sketches};
use crate::syntax::ast::*;
use std::collections::{HashMap, HashSet};

/// 連結性検査の許容誤差。
const EPS: f64 = 1e-9;

/// DSL が構文の一部として使う builtin 名。binding 名としては使えない。
const RESERVED_HEADS: [&str; 6] = ["p2", "line", "polygon", "segments", "circle", "translate2d"];

/// export された field の静的情報 (cross-sketch 参照の解決に使う)。
#[derive(Clone, Copy, Debug)]
enum Export {
    /// var / let。値は RHS 検査エラー時 `None`。
    Scalar(Option<f64>),
    /// 点束縛。値は座標が静的に決まらないとき `None`。
    Point(Option<P2>),
    /// 線分・polygon・circle (スカラーとしては参照不可)。
    Geometry,
}

/// cross-sketch 参照の解決テーブル。sketch を依存順に検査しながら埋める。
struct ExportScope<'a> {
    /// 全 top-level sketch 名 (未検査・循環含む)。
    sketch_names: HashSet<&'a str>,
    /// 検査済み sketch の export (field 名 → 静的情報)。
    exports: HashMap<&'a str, HashMap<&'a str, Export>>,
    /// 参照循環 (自己参照含む) で依存順に評価できなかった sketch。
    /// 参照は追加エラーを出さず未解決値として扱う (循環診断は別に出る)。
    unresolved: HashSet<&'a str>,
}

pub fn check_module(m: &Module) -> Vec<Diagnostic> {
    let mut diag = Vec::new();
    let sketches = top_sketches(m);
    let refs: Vec<&TopSketch> = sketches.iter().collect();
    let order = sketch_topo(&refs);
    if !order.cycle.is_empty() {
        let span = sketches
            .iter()
            .find(|s| s.name == order.cycle[0])
            .map(|s| s.span)
            .unwrap_or_else(Span::empty);
        let mut message = format!(
            "sketch 間の参照が循環しています (循環: {})",
            order.cycle.join(", ")
        );
        if !order.dependent.is_empty() {
            message.push_str(&format!(
                " — {} は循環に依存しているため解決できません",
                order.dependent.join(", ")
            ));
        }
        diag.push(Diagnostic::SketchDsl { span, message });
    }
    let by_name: HashMap<&str, usize> = sketches
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name, i))
        .collect();
    let mut scope = ExportScope {
        sketch_names: sketches.iter().map(|s| s.name).collect(),
        exports: HashMap::new(),
        unresolved: order
            .cycle
            .iter()
            .chain(order.dependent.iter())
            .copied()
            .collect(),
    };
    // 依存順に検査し、export の静的値を後続の sketch へ引き渡す。
    // 循環に関与する sketch も (他のエラーを報告するため) 検査はする。
    let mut checked = vec![false; sketches.len()];
    for name in order
        .order
        .iter()
        .chain(order.cycle.iter())
        .chain(order.dependent.iter())
    {
        let i = by_name[name];
        checked[i] = true;
        let s = &sketches[i];
        let exports = check_sketch(Some(s.name), s.bindings, s.body, &scope, &mut diag);
        scope.exports.insert(s.name, exports);
    }
    // 同名重複で shadow された sketch ブロックも検査だけは行う
    // (重複自体は sema::duplicates がエラーにする)。
    for (i, s) in sketches.iter().enumerate() {
        if !checked[i] {
            check_sketch(Some(s.name), s.bindings, s.body, &scope, &mut diag);
        }
    }
    // top-level 以外 (let 式内など) に埋まった sketch ブロックも検査する。
    for d in &m.decls {
        match d {
            Decl::Value(v) => {
                // top-level sketch binding の body は上で検査済み。
                if v.params.is_empty() && matches!(v.body, Expr::Sketch { .. }) {
                    continue;
                }
                find_sketches(&v.body, &scope, &mut diag);
            }
            Decl::Slider(s) => find_sketches(&s.body, &scope, &mut diag),
            _ => {}
        }
    }
    diag
}

/// 式ツリーから sketch ブロックを探して検査する。sketch の内部には再帰しない
/// (内部の制約は `check_sketch` が見る)。
fn find_sketches<'a>(e: &'a Expr, scope: &ExportScope<'a>, diag: &mut Vec<Diagnostic>) {
    match e {
        Expr::Sketch { bindings, body, .. } => {
            check_sketch(None, bindings, body, scope, diag);
        }
        Expr::Var { .. } | Expr::Ctor { .. } | Expr::Lit(..) | Expr::Error(_) => {}
        Expr::List(items, _) => items.iter().for_each(|x| find_sketches(x, scope, diag)),
        Expr::Record(fields, _) => fields
            .iter()
            .for_each(|f| find_sketches(&f.value, scope, diag)),
        Expr::RecordUpdate { base, updates, .. } => {
            find_sketches(base, scope, diag);
            updates
                .iter()
                .for_each(|f| find_sketches(&f.value, scope, diag));
        }
        Expr::Field { receiver, .. } => find_sketches(receiver, scope, diag),
        Expr::App { func, arg, .. } => {
            find_sketches(func, scope, diag);
            find_sketches(arg, scope, diag);
        }
        Expr::Lambda { body, .. } => find_sketches(body, scope, diag),
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .for_each(|b| find_sketches(&b.body, scope, diag));
            find_sketches(body, scope, diag);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            find_sketches(cond, scope, diag);
            find_sketches(then_branch, scope, diag);
            find_sketches(else_branch, scope, diag);
        }
        Expr::Case {
            scrutinee, arms, ..
        } => {
            find_sketches(scrutinee, scope, diag);
            for a in arms {
                if let Some(g) = &a.guard {
                    find_sketches(g, scope, diag);
                }
                find_sketches(&a.body, scope, diag);
            }
        }
        Expr::BinOp { left, right, .. } => {
            find_sketches(left, scope, diag);
            find_sketches(right, scope, diag);
        }
        Expr::Negate(inner, _) => find_sketches(inner, scope, diag),
        Expr::Range { lo, hi, .. } => {
            find_sketches(lo, scope, diag);
            find_sketches(hi, scope, diag);
        }
    }
}

/// binding 名 → 束縛の分類。静的値は検査エラー時 `None`。
#[derive(Clone, Debug)]
enum Entry {
    Scalar(Option<f64>),
    Point(Option<P2>),
    Segment(Option<Seg2>),
    /// polygon / circle。
    Shape,
}

struct Ctx<'a, 'd> {
    env: HashMap<&'a str, Entry>,
    /// 検査中の top-level sketch 名 (自己参照の検出用)。入れ子 sketch では `None`。
    current: Option<&'a str>,
    scope: &'d ExportScope<'a>,
    diag: &'d mut Vec<Diagnostic>,
}

impl Ctx<'_, '_> {
    fn err(&mut self, span: Span, message: impl Into<String>) {
        self.diag.push(Diagnostic::SketchDsl {
            span,
            message: message.into(),
        });
    }
}

/// sketch ブロック 1 つを検査し、body record の export 情報を返す。
fn check_sketch<'a>(
    current: Option<&'a str>,
    bindings: &'a [SketchBinding],
    body: &'a Expr,
    scope: &ExportScope<'a>,
    diag: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, Export> {
    let mut cx = Ctx {
        env: HashMap::new(),
        current,
        scope,
        diag,
    };
    for b in bindings {
        if RESERVED_HEADS.contains(&b.name.as_str()) {
            cx.err(
                b.name_span,
                format!("`{}` は sketch 内で予約された名前です", b.name),
            );
        } else if cx.env.contains_key(b.name.as_str()) {
            cx.err(b.name_span, format!("`{}` は既に定義されています", b.name));
        }
        let entry = match b.kind {
            SketchBindKind::Var => {
                let v = check_var_rhs(&mut cx, &b.body);
                Entry::Scalar(require_finite(&mut cx, &b.body, v))
            }
            SketchBindKind::Let => {
                let v = check_scalar(&mut cx, &b.body);
                Entry::Scalar(require_finite(&mut cx, &b.body, v))
            }
            SketchBindKind::Bare => check_geometry(&mut cx, &b.body),
        };
        cx.env.insert(b.name.as_str(), entry);
    }
    check_body(&mut cx, body);
    export_map(bindings, body)
        .into_iter()
        .map(|(field, binding)| {
            let export = match cx.env.get(binding) {
                Some(Entry::Scalar(v)) => Export::Scalar(*v),
                Some(Entry::Point(p)) => Export::Point(*p),
                _ => Export::Geometry,
            };
            (field, export)
        })
        .collect()
}

/// 静的値が非有限 (Inf/NaN) ならエラーにして未解決へ落とす
/// (幾何座標や export 経由で manifold に非有限値が流れるのを防ぐ)。
fn require_finite(cx: &mut Ctx, e: &Expr, v: Option<f64>) -> Option<f64> {
    match v {
        Some(x) if !x.is_finite() => {
            cx.err(span_of(e), format!("値が有限になりません ({x})"));
            None
        }
        other => other,
    }
}

/// `var x = <符号付き Float リテラル>` の右辺検査。
fn check_var_rhs(cx: &mut Ctx, e: &Expr) -> Option<f64> {
    match signed_float_lit(e) {
        Some(v) => Some(v),
        None => {
            cx.err(
                span_of(e),
                "var の右辺は Float リテラルのみ書けます (例: `var x = 3.0`)。計算式は let で書けます (ドラッグは参照先の var に伝播します)",
            );
            None
        }
    }
}

/// 符号付き Float リテラル (`3.0` / `-3.0`) なら値を返す。
fn signed_float_lit(e: &Expr) -> Option<f64> {
    match e {
        Expr::Lit(Lit::Float(v), _) => Some(*v),
        Expr::Negate(inner, _) => match inner.as_ref() {
            Expr::Lit(Lit::Float(v), _) => Some(-v),
            _ => None,
        },
        _ => None,
    }
}

/// スカラー式 (リテラル / 既出スカラー名 / 四則演算 / 単項マイナス) の検査 + 静的評価。
fn check_scalar(cx: &mut Ctx, e: &Expr) -> Option<f64> {
    match e {
        Expr::Lit(Lit::Float(v), _) => Some(*v),
        Expr::Lit(Lit::Int(_), span) => {
            cx.err(
                *span,
                "Int リテラルは使えません。Float で書いてください (例: `3.0`)",
            );
            None
        }
        Expr::Var {
            module: None,
            name,
            span,
        } => match cx.env.get(name.as_str()) {
            Some(Entry::Scalar(v)) => *v,
            Some(_) => {
                cx.err(*span, format!("`{name}` はスカラーではありません"));
                None
            }
            None => {
                cx.err(
                    *span,
                    format!(
                        "未定義の名前 `{name}` (他 sketch の値は `sketch名.field` で参照します)"
                    ),
                );
                None
            }
        },
        Expr::Field { .. } => check_scalar_field(cx, e),
        Expr::Negate(inner, _) => Some(-(check_scalar(cx, inner)?)),
        Expr::BinOp {
            op,
            left,
            right,
            span,
        } => {
            let lv = check_scalar(cx, left);
            match op {
                BinOp::Add => Some(lv? + check_scalar(cx, right)?),
                BinOp::Sub => Some(lv? - check_scalar(cx, right)?),
                BinOp::Mul => Some(lv? * check_scalar(cx, right)?),
                BinOp::Div => match signed_float_lit(right) {
                    Some(rv) if rv != 0.0 => Some(lv? / rv),
                    Some(_) => {
                        cx.err(span_of(right), "`/` の右辺に 0 は書けません");
                        None
                    }
                    None => {
                        cx.err(
                            span_of(right),
                            "`/` の右辺は 0 でない Float リテラルのみ書けます (逆評価のため)",
                        );
                        None
                    }
                },
                _ => {
                    cx.err(*span, "スカラー式で使える演算子は + - * / のみです");
                    None
                }
            }
        }
        Expr::Sketch { span, .. } => {
            cx.err(*span, "sketch ブロックは入れ子にできません");
            None
        }
        Expr::App { .. } => match app_spine(e) {
            (Some(f), args) if args.len() == 1 && MATH_FNS.contains(&f) => {
                let v = check_scalar(cx, args[0])?;
                match eval_math_fn(f, v).expect("MATH_FNS で判定済み") {
                    Ok(r) => Some(r),
                    Err(msg) => {
                        cx.err(span_of(e), msg);
                        None
                    }
                }
            }
            _ => {
                cx.err(
                    span_of(e),
                    "スカラー式で使える関数は sqrt / sin / cos / tan (引数 1 個) のみです",
                );
                None
            }
        },
        _ => {
            cx.err(
                span_of(e),
                "この式はスカラー式として使えません (リテラル / スカラー名 / 四則演算 / sqrt / sin / cos / tan のみ)",
            );
            None
        }
    }
}

/// cross-sketch 参照 (`A.f` / `A.pt.x`) のスカラー検査 + 静的評価。
fn check_scalar_field(cx: &mut Ctx, e: &Expr) -> Option<f64> {
    let Expr::Field {
        receiver,
        name: field,
        span,
    } = e
    else {
        unreachable!("Field 以外は check_scalar が処理する");
    };
    match receiver.as_ref() {
        // `A.f` — export されたスカラー
        Expr::Var {
            module: None,
            name: sk,
            span: rspan,
        } => match lookup_export(cx, sk, field, *rspan, *span)? {
            Export::Scalar(v) => v,
            Export::Point(_) => {
                cx.err(
                    *span,
                    format!(
                        "`{sk}.{field}` は点です (`{sk}.{field}.x` / `.y` で座標を参照してください)"
                    ),
                );
                None
            }
            Export::Geometry => {
                cx.err(*span, format!("`{sk}.{field}` はスカラーではありません"));
                None
            }
        },
        // `A.pt.x` — export された点の座標
        Expr::Field {
            receiver: inner,
            name: ptfield,
            span: pspan,
        } => {
            let Expr::Var {
                module: None,
                name: sk,
                span: rspan,
            } = inner.as_ref()
            else {
                cx.err(
                    *span,
                    "この field 参照はスカラー式として使えません (`sketch名.field` / `sketch名.点field.x|y` のみ)",
                );
                return None;
            };
            if field != "x" && field != "y" {
                cx.err(
                    *span,
                    format!("点の座標 field は x / y のみです (`.{field}`)"),
                );
                return None;
            }
            match lookup_export(cx, sk, ptfield, *rspan, *pspan)? {
                Export::Point(p) => p.map(|p| if field == "x" { p.x } else { p.y }),
                Export::Scalar(_) => {
                    cx.err(
                        *pspan,
                        format!("`{sk}.{ptfield}` はスカラーです (座標 field はありません)"),
                    );
                    None
                }
                Export::Geometry => {
                    cx.err(*pspan, format!("`{sk}.{ptfield}` は点ではありません"));
                    None
                }
            }
        }
        Expr::Var {
            module: Some(_), ..
        } => {
            cx.err(
                *span,
                "別モジュールの sketch は参照できません (参照できるのは同一モジュールの sketch のみ)",
            );
            None
        }
        _ => {
            cx.err(
                *span,
                "この field 参照はスカラー式として使えません (`sketch名.field` / `sketch名.点field.x|y` のみ)",
            );
            None
        }
    }
}

/// receiver が参照可能な sketch であることを検査して export を引く。
/// 循環で未解決の sketch への参照は追加エラーを出さず `None` (未解決) を返す。
fn lookup_export(cx: &mut Ctx, sk: &str, field: &str, rspan: Span, span: Span) -> Option<Export> {
    if cx.env.contains_key(sk) {
        cx.err(
            rspan,
            format!("`{sk}` はブロック内の束縛です (sketch 参照には使えません)"),
        );
        return None;
    }
    if cx.current == Some(sk) {
        cx.err(
            rspan,
            format!("`{sk}` は自分自身です (自 sketch の値は binding 名で直接参照してください)"),
        );
        return None;
    }
    if !cx.scope.sketch_names.contains(sk) {
        cx.err(
            rspan,
            format!(
                "`{sk}` は同一モジュールの sketch ではありません (参照できるのは top-level sketch の export のみ)"
            ),
        );
        return None;
    }
    if cx.scope.unresolved.contains(sk) {
        return None;
    }
    match cx.scope.exports.get(sk).and_then(|m| m.get(field)) {
        Some(e) => Some(*e),
        None => {
            cx.err(
                span,
                format!("sketch `{sk}` は `{field}` を export していません"),
            );
            None
        }
    }
}

/// 幾何束縛 (キーワード無し) の右辺検査。
fn check_geometry(cx: &mut Ctx, e: &Expr) -> Entry {
    let (head, args) = app_spine(e);
    match head {
        Some("p2") if args.len() == 2 => {
            let x = check_scalar(cx, args[0]);
            let y = check_scalar(cx, args[1]);
            Entry::Point(x.zip(y).map(|(x, y)| p2(x, y)))
        }
        Some("line") if args.len() == 2 => {
            let a = check_point_ref(cx, args[0]);
            let b = check_point_ref(cx, args[1]);
            Entry::Segment(a.zip(b).map(|(a, b)| Seg2 { a, b }))
        }
        Some("polygon") if args.len() == 1 => {
            check_polygon_arg(cx, args[0]);
            Entry::Shape
        }
        Some("circle") if args.len() == 1 => {
            check_scalar(cx, args[0]);
            Entry::Shape
        }
        _ => {
            // `circle r |> translate2d (p2 ..) (p2 ..)` の移動付き circle
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
                    check_scalar(cx, largs[0]);
                    check_point_ref(cx, rargs[0]);
                    check_point_ref(cx, rargs[1]);
                    return Entry::Shape;
                }
            }
            if let Expr::Sketch { span, .. } = e {
                cx.err(*span, "sketch ブロックは入れ子にできません");
            } else {
                cx.err(
                    span_of(e),
                    "幾何束縛の右辺は `p2 x y` / `line a b` / `polygon [...]` / `circle r` (+ `|> translate2d`) のみ書けます",
                );
            }
            Entry::Shape
        }
    }
}

/// 点参照: 点 binding 名か `p2 x y`。
fn check_point_ref(cx: &mut Ctx, e: &Expr) -> Option<P2> {
    match e {
        Expr::Var {
            module: None,
            name,
            span,
        } => match cx.env.get(name.as_str()) {
            Some(Entry::Point(v)) => *v,
            Some(_) => {
                cx.err(*span, format!("`{name}` は点 (p2) ではありません"));
                None
            }
            None => {
                cx.err(
                    *span,
                    format!(
                        "未定義の名前 `{name}` (他 sketch の値は `sketch名.field` で参照します)"
                    ),
                );
                None
            }
        },
        _ => {
            let (head, args) = app_spine(e);
            if head == Some("p2") && args.len() == 2 {
                let x = check_scalar(cx, args[0]);
                let y = check_scalar(cx, args[1]);
                x.zip(y).map(|(x, y)| p2(x, y))
            } else {
                cx.err(
                    span_of(e),
                    "点の位置は点 binding 名か `(p2 x y)` で書いてください",
                );
                None
            }
        }
    }
}

/// 線分参照: 線分 binding 名か `line a b`。
fn check_segment_item(cx: &mut Ctx, e: &Expr) -> Option<Seg2> {
    match e {
        Expr::Var {
            module: None,
            name,
            span,
        } => match cx.env.get(name.as_str()) {
            Some(Entry::Segment(v)) => *v,
            Some(_) => {
                cx.err(*span, format!("`{name}` は線分 (line) ではありません"));
                None
            }
            None => {
                cx.err(
                    *span,
                    format!(
                        "未定義の名前 `{name}` (他 sketch の値は `sketch名.field` で参照します)"
                    ),
                );
                None
            }
        },
        _ => {
            let (head, args) = app_spine(e);
            if head == Some("line") && args.len() == 2 {
                let a = check_point_ref(cx, args[0]);
                let b = check_point_ref(cx, args[1]);
                a.zip(b).map(|(a, b)| Seg2 { a, b })
            } else {
                cx.err(
                    span_of(e),
                    "polygon の要素は線分 binding 名か `line a b` で書いてください",
                );
                None
            }
        }
    }
}

/// polygon の引数: `[<線分>, ...]` か `(segments [<点>, ...])`。
fn check_polygon_arg(cx: &mut Ctx, e: &Expr) {
    match e {
        Expr::List(items, span) => {
            let mut segs: Vec<Seg2> = Vec::new();
            let mut all_known = true;
            for it in items {
                match check_segment_item(cx, it) {
                    Some(s) => segs.push(s),
                    None => all_known = false,
                }
            }
            if all_known && segs.len() >= 2 {
                check_chain(cx, &segs, *span);
            }
        }
        _ => {
            let (head, args) = app_spine(e);
            if head == Some("segments") && args.len() == 1 {
                match args[0] {
                    Expr::List(items, _) => {
                        for it in items {
                            check_point_ref(cx, it);
                        }
                    }
                    other => cx.err(
                        span_of(other),
                        "segments の引数は `[p2 .., ...]` のリストリテラルのみ書けます",
                    ),
                }
            } else {
                cx.err(
                    span_of(e),
                    "polygon の引数は `[line .., ...]` のリストか `(segments [p2 .., ...])` のみ書けます",
                );
            }
        }
    }
}

/// 線分列の連結性 + 閉路検査。
fn check_chain(cx: &mut Ctx, segs: &[Seg2], span: Span) {
    let near = |a: P2, b: P2| (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS;
    for w in segs.windows(2) {
        if !near(w[0].b, w[1].a) {
            cx.err(
                span,
                "polygon の線分が連結していません (前の線分の終点 = 次の線分の始点 にしてください)",
            );
            return;
        }
    }
    let last = segs[segs.len() - 1];
    let first = segs[0];
    if !near(last.b, first.a) {
        cx.err(
            span,
            "polygon が閉じていません (最後の線分の終点 = 最初の線分の始点 にしてください)",
        );
    }
}

/// body: `{ f = 束縛名, ... }` の record か単一の幾何名。record の field は
/// 幾何に加えて var / let (スカラー) も書ける (他 sketch から参照できる export になる)。
fn check_body(cx: &mut Ctx, body: &Expr) {
    match body {
        Expr::Record(fields, _) => {
            for f in fields {
                match &f.value {
                    Expr::Var {
                        module: None,
                        name,
                        span,
                    } => {
                        if !cx.env.contains_key(name.as_str()) {
                            cx.err(*span, format!("未定義の名前 `{name}`"));
                        }
                    }
                    other => cx.err(
                        span_of(other),
                        "body の field は sketch 内の束縛の名前のみ書けます",
                    ),
                }
            }
        }
        Expr::Var {
            module: None,
            name,
            span,
        } => match cx.env.get(name.as_str()) {
            Some(Entry::Shape | Entry::Point(_) | Entry::Segment(_)) => {}
            Some(Entry::Scalar(_)) => cx.err(
                *span,
                format!(
                    "単一名 body は幾何束縛のみ書けます (`{name}` はスカラー)。スカラーを export するには record にしてください"
                ),
            ),
            None => cx.err(*span, format!("未定義の名前 `{name}`")),
        },
        _ => cx.err(
            span_of(body),
            "body は `{ 名前 = 束縛名, ... }` の record か単一の幾何名のみ書けます",
        ),
    }
}

fn span_of(e: &Expr) -> Span {
    e.span()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse::parse;

    fn check_src(src: &str) -> Vec<Diagnostic> {
        let m = parse(src).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
        check_module(&m)
    }

    fn assert_ok(src: &str) {
        let diags = check_src(src);
        assert!(diags.is_empty(), "expected no diags, got: {diags:?}");
    }

    fn assert_err_contains(src: &str, needle: &str) {
        let diags = check_src(src);
        assert!(
            diags.iter().any(|d| d.message().contains(needle)),
            "expected err containing `{needle}`, got: {diags:?}"
        );
    }

    #[test]
    fn minimal_sketch_ok() {
        assert_ok(
            "sk = sketch\n    var x1 = 0.0\n    let y1 = 3.0\n    poly1 = polygon (segments [p2 x1 y1, p2 4.0 y1, p2 x1 7.0])\n    in { poly1 = poly1 }\nend\n",
        );
    }

    #[test]
    fn line_based_polygon_ok() {
        assert_ok(
            "sk = sketch\n    v1 = p2 0.0 0.0\n    v2 = p2 4.0 0.0\n    v3 = p2 4.0 3.0\n    l1 = line v1 v2\n    poly1 = polygon [l1, line v2 v3, line v3 v1]\n    in poly1\nend\n",
        );
    }

    #[test]
    fn circle_forms_ok() {
        assert_ok(
            "sk = sketch\n    var r = 2.0\n    circ1 = circle r\n    circ2 = circle 3.0 |> translate2d (p2 0.0 0.0) (p2 4.0 5.0)\n    in { circ1 = circ1, circ2 = circ2 }\nend\n",
        );
    }

    #[test]
    fn var_rhs_must_be_literal() {
        assert_err_contains(
            "sk = sketch\n    var x = 1.0 + 2.0\n    p = p2 x 0.0\n    in p\nend\n",
            "var の右辺は Float リテラル",
        );
    }

    #[test]
    fn int_literal_rejected() {
        assert_err_contains(
            "sk = sketch\n    var x = 3.0\n    p = p2 x 1\n    in p\nend\n",
            "Int リテラル",
        );
    }

    #[test]
    fn div_rhs_must_be_nonzero_literal() {
        assert_err_contains(
            "sk = sketch\n    var x = 4.0\n    let y = x / 0.0\n    p = p2 x y\n    in p\nend\n",
            "0 は書けません",
        );
        assert_err_contains(
            "sk = sketch\n    var x = 4.0\n    let z = 2.0\n    let y = x / z\n    p = p2 x y\n    in p\nend\n",
            "Float リテラルのみ",
        );
    }

    #[test]
    fn math_fns_in_scalar_ok() {
        assert_ok(
            "sk = sketch\n    var x = 4.0\n    let y = sqrt (x * x + 9.0)\n    let z = sin 30.0 + cos 60.0 - tan 45.0\n    p = p2 y z\n    in p\nend\n",
        );
    }

    #[test]
    fn sqrt_negative_rejected() {
        assert_err_contains(
            "sk = sketch\n    let y = sqrt (0.0 - 1.0)\n    p = p2 y 0.0\n    in p\nend\n",
            "平方根は計算できません",
        );
    }

    #[test]
    fn tan_undefined_at_90_rejected() {
        assert_err_contains(
            "sk = sketch\n    let y = tan 270.0\n    p = p2 y 0.0\n    in p\nend\n",
            "では定義されません",
        );
    }

    #[test]
    fn non_math_fn_in_scalar_rejected() {
        assert_err_contains(
            "sk = sketch\n    let y = cube 1.0 1.0 1.0\n    p = p2 y 0.0\n    in p\nend\n",
            "スカラー式で使える関数は sqrt / sin / cos / tan",
        );
    }

    #[test]
    fn external_reference_rejected() {
        // 通常のトップレベル宣言は (スカラーでも) sketch から参照できない。
        assert_err_contains(
            "w = 10.0\nsk = sketch\n    p = p2 w 0.0\n    in p\nend\n",
            "未定義の名前 `w`",
        );
    }

    #[test]
    fn cross_sketch_scalar_ref_ok() {
        // export された var / let は宣言順に関係なく他 sketch から参照できる。
        // sketch 内 let のオペランドにも使える。
        assert_ok(
            "sk = sketch\n    let y = other.w + 1.0\n    p = p2 other.z1 y\n    in p\nend\nother = sketch\n    var z1 = 50.0\n    let w = z1 * 2.0\n    q = p2 0.0 z1\n    in { q = q, z1, w }\nend\n",
        );
    }

    #[test]
    fn cross_sketch_var_shared_by_two_sketches_ok() {
        assert_ok(
            "base = sketch\n    var z1 = 50.0\n    o = p2 0.0 z1\n    in { o = o, z1 }\nend\nskA = sketch\n    p = p2 0.0 base.z1\n    in p\nend\nskB = sketch\n    q = p2 1.0 base.z1\n    in q\nend\n",
        );
    }

    #[test]
    fn cross_sketch_point_axis_ok() {
        assert_ok(
            "skA = sketch\n    anchor = p2 5.0 40.0\n    in { anchor }\nend\nskB = sketch\n    p = p2 skA.anchor.x skA.anchor.y\n    in p\nend\n",
        );
    }

    #[test]
    fn cross_sketch_value_used_in_connectivity_check() {
        // export 値が連結性検査まで届くこと: skA.w = 4.0 なので 5.0 と繋がらない。
        assert_err_contains(
            "skA = sketch\n    var w = 4.0\n    pw = p2 w 0.0\n    in { pw = pw, w }\nend\nskB = sketch\n    poly1 = polygon [line (p2 0.0 0.0) (p2 skA.w 0.0), line (p2 5.0 0.0) (p2 0.0 0.0)]\n    in poly1\nend\n",
            "連結していません",
        );
    }

    #[test]
    fn cross_sketch_unexported_rejected() {
        assert_err_contains(
            "skA = sketch\n    var z1 = 50.0\n    p = p2 0.0 z1\n    in { p = p }\nend\nskB = sketch\n    q = p2 0.0 skA.z1\n    in q\nend\n",
            "`z1` を export していません",
        );
    }

    #[test]
    fn cross_sketch_geometry_export_rejected_as_scalar() {
        assert_err_contains(
            "skA = sketch\n    poly1 = polygon (segments [p2 0.0 0.0, p2 4.0 0.0, p2 0.0 3.0])\n    in { poly1 = poly1 }\nend\nskB = sketch\n    q = p2 0.0 skA.poly1\n    in q\nend\n",
            "`skA.poly1` はスカラーではありません",
        );
    }

    #[test]
    fn cross_sketch_point_direct_ref_rejected() {
        assert_err_contains(
            "skA = sketch\n    anchor = p2 5.0 40.0\n    in { anchor }\nend\nskB = sketch\n    q = p2 0.0 skA.anchor\n    in q\nend\n",
            "は点です",
        );
    }

    #[test]
    fn cross_sketch_point_axis_only_x_y() {
        assert_err_contains(
            "skA = sketch\n    anchor = p2 5.0 40.0\n    in { anchor }\nend\nskB = sketch\n    q = p2 0.0 skA.anchor.z\n    in q\nend\n",
            "x / y のみ",
        );
    }

    #[test]
    fn cross_sketch_axis_on_scalar_rejected() {
        assert_err_contains(
            "skA = sketch\n    var w = 4.0\n    p = p2 w 0.0\n    in { p = p, w }\nend\nskB = sketch\n    q = p2 0.0 skA.w.x\n    in q\nend\n",
            "スカラーです (座標 field はありません)",
        );
    }

    #[test]
    fn cross_sketch_non_sketch_receiver_rejected() {
        assert_err_contains(
            "w = 10.0\nsk = sketch\n    p = p2 w.x 0.0\n    in p\nend\n",
            "同一モジュールの sketch ではありません",
        );
    }

    #[test]
    fn cross_sketch_self_reference_rejected() {
        assert_err_contains(
            "sk = sketch\n    var v = 1.0\n    let a = sk.v + 1.0\n    p = p2 a 0.0\n    in { p = p, v }\nend\n",
            "自分自身",
        );
    }

    #[test]
    fn cross_sketch_local_shadow_receiver_rejected() {
        // 参照先 sketch と同名のブロック内束縛があると sketch 参照には使えない。
        assert_err_contains(
            "other = sketch\n    var w = 1.0\n    o = p2 w 0.0\n    in { o = o, w }\nend\nsk = sketch\n    let other = 1.0\n    p = p2 other.w 0.0\n    in p\nend\n",
            "ブロック内の束縛です",
        );
    }

    #[test]
    fn cross_sketch_cycle_rejected() {
        assert_err_contains(
            "skA = sketch\n    let a = skB.b + 1.0\n    p = p2 a 0.0\n    in { p = p, a }\nend\nskB = sketch\n    let b = skA.a + 1.0\n    q = p2 b 0.0\n    in { q = q, b }\nend\n",
            "循環しています",
        );
    }

    #[test]
    fn cycle_diag_separates_members_from_dependents() {
        // skZ は循環に依存しているだけ: メッセージで循環メンバーと区別される。
        let src = "skZ = sketch\n    p = p2 skA.a 0.0\n    in p\nend\nskA = sketch\n    let a = skB.b + 1.0\n    q = p2 a 0.0\n    in { q = q, a }\nend\nskB = sketch\n    let b = skA.a + 1.0\n    r = p2 b 0.0\n    in { r = r, b }\nend\n";
        assert_err_contains(src, "循環: skA, skB");
        assert_err_contains(src, "skZ は循環に依存しているため");
        // span は循環メンバー (skA) を指す
        let diags = check_src(src);
        let cyc = diags
            .iter()
            .find(|d| d.message().contains("循環"))
            .expect("cycle diag");
        let m = parse(src).unwrap();
        let ska_span = crate::sketch::top_sketches(&m)
            .iter()
            .find(|s| s.name == "skA")
            .map(|s| s.span)
            .unwrap();
        assert_eq!(cyc.span(), ska_span);
    }

    #[test]
    fn shadowed_duplicate_sketch_is_still_checked() {
        // 同名重複の先頭 (shadow される側) の DSL 違反も報告される
        // (重複そのものは sema::duplicates がエラーにする)。
        assert_err_contains(
            "skA = sketch\n    let y = cube 1.0 1.0 1.0\n    p = p2 y 0.0\n    in p\nend\nskA = sketch\n    q = p2 1.0 1.0\n    in q\nend\n",
            "スカラー式で使える関数は",
        );
    }

    #[test]
    fn non_finite_scalar_rejected() {
        let big = "999999999999999999999999999999999999999.0"; // ~1e39
        assert_err_contains(
            &format!(
                "sk = sketch\n    let b1 = {big} * {big}\n    let b2 = b1 * b1\n    let b3 = b2 * b2\n    p = p2 b3 0.0\n    in p\nend\n"
            ),
            "有限になりません",
        );
    }

    #[test]
    fn forward_reference_rejected() {
        assert_err_contains(
            "sk = sketch\n    p = p2 x 0.0\n    var x = 1.0\n    in p\nend\n",
            "未定義の名前 `x`",
        );
    }

    #[test]
    fn if_rejected_in_scalar() {
        assert_err_contains(
            "sk = sketch\n    let y = if True then 1.0 else 2.0\n    p = p2 y 0.0\n    in p\nend\n",
            "スカラー式として使えません",
        );
    }

    #[test]
    fn disconnected_polygon_rejected() {
        assert_err_contains(
            "sk = sketch\n    poly1 = polygon [line (p2 0.0 0.0) (p2 1.0 0.0), line (p2 5.0 5.0) (p2 6.0 5.0)]\n    in poly1\nend\n",
            "連結していません",
        );
    }

    #[test]
    fn open_polygon_rejected() {
        assert_err_contains(
            "sk = sketch\n    poly1 = polygon [line (p2 0.0 0.0) (p2 1.0 0.0), line (p2 1.0 0.0) (p2 1.0 1.0)]\n    in poly1\nend\n",
            "閉じていません",
        );
    }

    #[test]
    fn body_must_be_record_or_name() {
        assert_err_contains(
            "sk = sketch\n    var r = 2.0\n    circ1 = circle r\n    in union2d circ1 circ1\nend\n",
            "body は",
        );
    }

    #[test]
    fn body_record_exports_scalar_ok() {
        assert_ok("sk = sketch\n    var x = 2.0\n    p = p2 x x\n    in { p = p, x = x }\nend\n");
    }

    #[test]
    fn single_name_body_cannot_be_scalar() {
        assert_err_contains(
            "sk = sketch\n    var x = 2.0\n    p = p2 x x\n    in x\nend\n",
            "単一名 body は幾何束縛のみ",
        );
    }

    #[test]
    fn duplicate_binding_rejected() {
        assert_err_contains(
            "sk = sketch\n    var x = 1.0\n    var x = 2.0\n    p = p2 x x\n    in p\nend\n",
            "既に定義されています",
        );
    }

    #[test]
    fn reserved_name_rejected() {
        assert_err_contains(
            "sk = sketch\n    var line = 1.0\n    p = p2 line line\n    in p\nend\n",
            "予約された名前",
        );
    }

    #[test]
    fn nested_sketch_rejected() {
        assert_err_contains(
            "sk = sketch\n    let y = sketch\n            var z = 1.0\n            q = p2 z z\n        in q\n        end\n    p = p2 y y\n    in p\nend\n",
            "入れ子",
        );
    }

    #[test]
    fn scalar_referencing_geometry_rejected() {
        assert_err_contains(
            "sk = sketch\n    p = p2 1.0 2.0\n    let y = p + 1.0\n    q = p2 y y\n    in q\nend\n",
            "スカラーではありません",
        );
    }

    #[test]
    fn sketch_nested_in_let_is_found() {
        assert_err_contains(
            "sk =\n    let\n        inner = sketch\n                var x = 1\n                p = p2 x x\n            in p\n            end\n    in\n    inner\n",
            "var の右辺は Float リテラル",
        );
    }
}
