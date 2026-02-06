use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take_until, take_while, take_while1},
    character::complete::{char, digit1, multispace1},
    combinator::{cut, map, map_res, opt, recognize, value},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated},
};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bound {
    pub value: i64,
    pub inclusive: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, PartialEq)]
pub enum Term {
    Var {
        name: String,
    },
    /// 範囲制約付き変数: min < X < max など
    RangeVar {
        name: String,
        min: Option<Bound>,
        max: Option<Bound>,
    },
    Number {
        value: i64,
    },
    /// 算術式: left op right
    ArithExpr {
        op: ArithOp,
        left: Box<Term>,
        right: Box<Term>,
    },
    Struct {
        functor: String,
        args: Vec<Term>,
    },
    List {
        items: Vec<Term>,
        tail: Option<Box<Term>>,
    },
}

#[derive(Clone, PartialEq)]
pub enum Clause {
    Fact(Term),
    Rule { head: Term, body: Vec<Term> },
}

impl fmt::Debug for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var { name } => write!(f, "{}", name),
            Term::RangeVar { name, min, max } => {
                if let Some(b) = min {
                    write!(f, "{} {} ", b.value, if b.inclusive { "<=" } else { "<" })?;
                }
                write!(f, "{}", name)?;
                if let Some(b) = max {
                    write!(f, " {} {}", if b.inclusive { "<=" } else { "<" }, b.value)?;
                }
                Ok(())
            }
            Term::Number { value } => write!(f, "{}", value),
            Term::ArithExpr { op, left, right } => {
                let op_str = match op {
                    ArithOp::Add => "+",
                    ArithOp::Sub => "-",
                    ArithOp::Mul => "*",
                    ArithOp::Div => "/",
                };
                write!(f, "({:?} {} {:?})", left, op_str, right)
            }
            Term::Struct { functor, args } => {
                write!(f, "{}", functor)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (idx, arg) in args.iter().enumerate() {
                        if idx > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{:?}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Term::List { items, tail } => {
                write!(f, "[")?;
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", item)?;
                }
                if let Some(tail) = tail {
                    if !items.is_empty() {
                        write!(f, " | ")?;
                    }
                    write!(f, "{:?}", tail)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl fmt::Debug for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clause::Fact(term) => write!(f, "{:?}.", term),
            Clause::Rule { head, body } => {
                write!(f, "{:?} :- ", head)?;
                for (idx, term) in body.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", term)?;
                }
                write!(f, ".")
            }
        }
    }
}

/// Termコンストラクタ
pub fn var(name: String) -> Term {
    Term::Var { name }
}

pub fn number(value: i64) -> Term {
    Term::Number { value }
}

pub fn struc(functor: String, args: Vec<Term>) -> Term {
    Term::Struct { functor, args }
}

pub fn list(items: Vec<Term>, tail: Option<Term>) -> Term {
    Term::List {
        items,
        tail: tail.map(Box::new),
    }
}

pub fn range_var(name: String, min: Option<Bound>, max: Option<Bound>) -> Term {
    Term::RangeVar { name, min, max }
}

pub fn arith_expr(op: ArithOp, left: Term, right: Term) -> Term {
    Term::ArithExpr {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

#[allow(unused)]
pub(super) fn v(name: impl Into<String>) -> Term {
    var(name.into())
}

#[allow(unused)]
pub(super) fn a(name: impl Into<String>) -> Term {
    struc(name.into(), vec![])
}

pub(super) type PResult<'a, T> = IResult<&'a str, T>;

// Whitespace and comments
fn line_comment(input: &str) -> PResult<'_, ()> {
    map(pair(tag("%"), opt(is_not("\n"))), |_| ()).parse(input)
}

fn block_comment(input: &str) -> PResult<'_, ()> {
    map((tag("/*"), cut(take_until("*/")), tag("*/")), |_| ()).parse(input)
}

fn space_or_comment1(input: &str) -> PResult<'_, ()> {
    value(
        (),
        many0(alt((value((), multispace1), line_comment, block_comment))),
    )
    .parse(input)
}

fn space_or_comment0(input: &str) -> PResult<'_, ()> {
    value(
        (),
        many0(alt((value((), multispace1), line_comment, block_comment))),
    )
    .parse(input)
}

fn ws<'a, F, O>(inner: F) -> impl Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>
where
    F: Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>,
{
    delimited(space_or_comment0, inner, space_or_comment0)
}

// Identifiers and atoms
fn is_atom_start(c: char) -> bool {
    c.is_ascii_lowercase()
}

fn is_id_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_var_start(c: char) -> bool {
    c.is_ascii_uppercase() || c == '_'
}

fn unquoted_atom(input: &str) -> PResult<'_, String> {
    map(
        recognize(pair(take_while1(is_atom_start), take_while(is_id_continue))),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

fn quoted_atom(input: &str) -> PResult<'_, String> {
    let escaped_char = preceded(
        char('\\'),
        alt((
            value('\'', char('\'')),
            value('\\', char('\\')),
            value('\n', char('n')),
            value('\t', char('t')),
        )),
    );
    let normal_char = map(is_not("'\\"), |s: &str| s.chars().collect::<Vec<_>>());

    map(
        delimited(
            char('\''),
            map(
                many0(alt((map(escaped_char, |c| vec![c]), normal_char))),
                |v: Vec<Vec<char>>| v.into_iter().flatten().collect::<String>(),
            ),
            cut(char('\'')),
        ),
        |s| s,
    )
    .parse(input)
}

fn atom(input: &str) -> PResult<'_, String> {
    alt((quoted_atom, unquoted_atom)).parse(input)
}

fn variable(input: &str) -> PResult<'_, String> {
    map(
        recognize(pair(take_while1(is_var_start), take_while(is_id_continue))),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

fn integer(input: &str) -> PResult<'_, i64> {
    map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| {
        s.parse::<i64>()
    })
    .parse(input)
}

// Terms
fn list_term(input: &str) -> PResult<'_, Term> {
    ws(delimited(
        char('['),
        map(
            (
                separated_list0(ws(char(',')), term),
                opt(preceded(ws(char('|')), term)),
            ),
            |(items, tail)| list(items, tail),
        ),
        cut(ws(char(']'))),
    ))
    .parse(input)
}

fn paren_term(input: &str) -> PResult<'_, Term> {
    delimited(ws(char('(')), term, cut(ws(char(')')))).parse(input)
}

fn number_term(input: &str) -> PResult<'_, Term> {
    map(ws(integer), number).parse(input)
}

/// 比較演算子 (<, <=, >, >=)
#[derive(Clone, Copy)]
enum CompOp {
    Lt,
    Le,
    Gt,
    Ge,
}

fn comp_op(input: &str) -> PResult<'_, CompOp> {
    ws(alt((
        map(tag("<="), |_| CompOp::Le),
        map(tag(">="), |_| CompOp::Ge),
        map(char('<'), |_| CompOp::Lt),
        map(char('>'), |_| CompOp::Gt),
    )))
    .parse(input)
}

/// range_var: `0 < X < 10`, `X < 10`, `0 < X`, `X > 0` など
fn range_var_term(input: &str) -> PResult<'_, Term> {
    // 左側: (num op)?
    let left_bound = opt((ws(integer), comp_op));
    // 変数名
    let var_name = ws(variable);
    // 右側: (op num)?
    let right_bound = opt((comp_op, ws(integer)));

    map(
        (left_bound, var_name, right_bound),
        |(left, name, right)| {
            let min = match left {
                Some((val, CompOp::Lt)) => Some(Bound {
                    value: val,
                    inclusive: false,
                }),
                Some((val, CompOp::Le)) => Some(Bound {
                    value: val,
                    inclusive: true,
                }),
                Some((_, CompOp::Gt | CompOp::Ge)) => return var(name),
                None => None,
            };

            let max = match right {
                Some((CompOp::Lt, val)) => Some(Bound {
                    value: val,
                    inclusive: false,
                }),
                Some((CompOp::Le, val)) => Some(Bound {
                    value: val,
                    inclusive: true,
                }),
                Some((CompOp::Gt | CompOp::Ge, _)) => return var(name),
                None => None,
            };

            if min.is_none() && max.is_none() {
                var(name)
            } else {
                range_var(name, min, max)
            }
        },
    )
    .parse(input)
}

#[allow(unused)]
fn var_term(input: &str) -> PResult<'_, Term> {
    map(ws(variable), var).parse(input)
}

fn atom_term(input: &str) -> PResult<'_, Term> {
    ws(map(
        pair(
            atom,
            opt(ws(delimited(
                char('('),
                separated_list0(ws(char(',')), term),
                cut(ws(char(')'))),
            ))),
        ),
        |(name, maybe_args)| match maybe_args {
            Some(args) => struc(name, args),
            None => struc(name, vec![]),
        },
    ))
    .parse(input)
}

fn primary_term(input: &str) -> PResult<'_, Term> {
    // range_var_term は number_term より先に試行（0 < X のような形式を正しくパースするため）
    alt((
        list_term,
        paren_term,
        range_var_term,
        number_term,
        atom_term,
    ))
    .parse(input)
}

fn mul_op(input: &str) -> PResult<'_, ArithOp> {
    ws(alt((
        map(char('*'), |_| ArithOp::Mul),
        map(char('/'), |_| ArithOp::Div),
    )))
    .parse(input)
}

fn add_op(input: &str) -> PResult<'_, ArithOp> {
    ws(alt((
        map(char('+'), |_| ArithOp::Add),
        map(char('-'), |_| ArithOp::Sub),
    )))
    .parse(input)
}

fn mul_expr(input: &str) -> PResult<'_, Term> {
    let (input, first) = primary_term(input)?;
    let (input, rest) = many0(pair(mul_op, primary_term)).parse(input)?;

    let result = rest
        .into_iter()
        .fold(first, |left, (op, right)| arith_expr(op, left, right));
    Ok((input, result))
}

fn add_expr(input: &str) -> PResult<'_, Term> {
    let (input, first) = mul_expr(input)?;
    let (input, rest) = many0(pair(add_op, mul_expr)).parse(input)?;

    let result = rest
        .into_iter()
        .fold(first, |left, (op, right)| arith_expr(op, left, right));
    Ok((input, result))
}

fn simple_term(input: &str) -> PResult<'_, Term> {
    add_expr(input)
}

pub(super) fn term(input: &str) -> PResult<'_, Term> {
    simple_term(input)
}

fn goals(input: &str) -> PResult<'_, Vec<Term>> {
    separated_list1(ws(char(',')), term).parse(input)
}

pub(super) fn clause_parser(input: &str) -> PResult<'_, Clause> {
    ws(terminated(
        alt((
            map(
                separated_pair(term, ws(tag(":-")), goals),
                |(head, body)| Clause::Rule { head, body },
            ),
            map(term, Clause::Fact),
        )),
        cut(ws(char('.'))),
    ))
    .parse(input)
}

pub fn program(input: &str) -> PResult<'_, Vec<Clause>> {
    ws(terminated(many0(clause_parser), opt(space_or_comment1))).parse(input)
}

pub fn database(input: &str) -> Result<Vec<Clause>, nom::Err<nom::error::Error<&str>>> {
    match program(input) {
        Ok((rest, clauses)) if rest.is_empty() => Ok(clauses),
        Ok((rest, _)) => Err(nom::Err::Error(nom::error::Error {
            input: rest,
            code: nom::error::ErrorKind::Fail,
        })),
        Err(e) => Err(e),
    }
}

pub fn query(input: &str) -> PResult<'_, Vec<Term>> {
    ws(terminated(goals, cut(ws(char('.'))))).parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_clause(src: &str, expected: Clause) {
        let (_, parsed) = clause_parser(src).unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_fact() {
        assert_clause(
            "parent(alice, bob).",
            Clause::Fact(struc("parent".to_string(), vec![a("alice"), a("bob")])),
        );
    }

    #[test]
    fn parse_rule() {
        assert_clause(
            "grandparent(X, Y) :- parent(X, Z), parent(Z, Y).",
            Clause::Rule {
                head: struc("grandparent".to_string(), vec![v("X"), v("Y")]),
                body: vec![
                    struc("parent".to_string(), vec![v("X"), v("Z")]),
                    struc("parent".to_string(), vec![v("Z"), v("Y")]),
                ],
            },
        );
    }

    #[test]
    fn parse_list() {
        assert_clause(
            "member(X, [X|_]).",
            Clause::Fact(struc(
                "member".to_string(),
                vec![v("X"), list(vec![v("X")], Some(v("_")))],
            )),
        );
    }

    #[test]
    fn parse_query_simple() {
        let src = "member(X, [1,2,3]).";
        let (_, qs) = query(src).unwrap();
        assert_eq!(
            qs,
            vec![struc(
                "member".to_string(),
                vec![v("X"), list(vec![number(1), number(2), number(3)], None),],
            )]
        );
    }

    #[test]
    fn parse_database() {
        let src = r#"
            % facts
            parent(alice, bob).
            parent(bob, carol).

            /* rule */
            grandparent(X, Y) :- parent(X, Z), parent(Z, Y).
        "#;
        let db = database(src).unwrap();
        assert_eq!(db.len(), 3);
    }

    #[test]
    fn test_struct() {
        let src = "parent(alice, f(nested)).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { functor, args } => {
                    assert_eq!(functor, "parent");
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn test_atom() {
        let src = "hello.";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { functor, args } => {
                    assert_eq!(functor, "hello");
                    assert_eq!(args.len(), 0);
                }
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn parse_range_var_both_bounds() {
        let src = "hoge(0<X<10).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { functor, args } => {
                    assert_eq!(functor, "hoge");
                    assert_eq!(args.len(), 1);
                    match &args[0] {
                        Term::RangeVar { name, min, max } => {
                            assert_eq!(name, "X");
                            assert_eq!(
                                *min,
                                Some(Bound {
                                    value: 0,
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
                        _ => panic!("Expected RangeVar, got {:?}", args[0]),
                    }
                }
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn parse_range_var_inclusive() {
        let src = "hoge(0<=X<=10).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { args, .. } => match &args[0] {
                    Term::RangeVar { name, min, max } => {
                        assert_eq!(name, "X");
                        assert_eq!(
                            *min,
                            Some(Bound {
                                value: 0,
                                inclusive: true
                            })
                        );
                        assert_eq!(
                            *max,
                            Some(Bound {
                                value: 10,
                                inclusive: true
                            })
                        );
                    }
                    _ => panic!("Expected RangeVar"),
                },
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn parse_range_var_left_only() {
        let src = "hoge(0<X).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { args, .. } => match &args[0] {
                    Term::RangeVar { name, min, max } => {
                        assert_eq!(name, "X");
                        assert_eq!(
                            *min,
                            Some(Bound {
                                value: 0,
                                inclusive: false
                            })
                        );
                        assert_eq!(*max, None);
                    }
                    _ => panic!("Expected RangeVar"),
                },
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn parse_range_var_right_only() {
        let src = "hoge(X<10).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Fact(term) => match &term {
                Term::Struct { args, .. } => match &args[0] {
                    Term::RangeVar { name, min, max } => {
                        assert_eq!(name, "X");
                        assert_eq!(*min, None);
                        assert_eq!(
                            *max,
                            Some(Bound {
                                value: 10,
                                inclusive: false
                            })
                        );
                    }
                    _ => panic!("Expected RangeVar"),
                },
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }

    #[test]
    fn parse_range_var_in_rule() {
        let src = "hoge(0<X<10) :- cube(X, X, X).";
        let (_, clause) = clause_parser(src).unwrap();

        match clause {
            Clause::Rule { head, body } => {
                match &head {
                    Term::Struct { functor, args } => {
                        assert_eq!(functor, "hoge");
                        assert!(matches!(&args[0], Term::RangeVar { .. }));
                    }
                    _ => panic!("Expected Struct"),
                }
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected Rule"),
        }
    }
}
