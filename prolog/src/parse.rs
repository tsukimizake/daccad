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
use std::rc::Rc;

/// Rcでラップされた項
pub type Term = Rc<TermInner>;

#[derive(Clone, PartialEq)]
pub enum TermInner {
    Var { name: String },
    Number { value: i64 },
    Struct { functor: String, args: Vec<Term> },
    List { items: Vec<Term>, tail: Option<Term> },
}

#[derive(Clone, PartialEq)]
pub enum Clause {
    Fact(Term),
    Rule { head: Term, body: Vec<Term> },
}

impl fmt::Debug for TermInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermInner::Var { name } => write!(f, "{}", name),
            TermInner::Number { value } => write!(f, "{}", value),
            TermInner::Struct { functor, args } => {
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
            TermInner::List { items, tail } => {
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
    Rc::new(TermInner::Var { name })
}

pub fn number(value: i64) -> Term {
    Rc::new(TermInner::Number { value })
}

pub fn struc(functor: String, args: Vec<Term>) -> Term {
    Rc::new(TermInner::Struct { functor, args })
}

pub fn list(items: Vec<Term>, tail: Option<Term>) -> Term {
    Rc::new(TermInner::List { items, tail })
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

fn simple_term(input: &str) -> PResult<'_, Term> {
    alt((list_term, paren_term, number_term, var_term, atom_term)).parse(input)
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
                vec![
                    v("X"),
                    list(vec![number(1), number(2), number(3)], None),
                ],
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
            Clause::Fact(term) => match term.as_ref() {
                TermInner::Struct { functor, args } => {
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
            Clause::Fact(term) => match term.as_ref() {
                TermInner::Struct { functor, args } => {
                    assert_eq!(functor, "hello");
                    assert_eq!(args.len(), 0);
                }
                _ => panic!("Expected Struct"),
            },
            _ => panic!("Expected Fact"),
        }
    }
}
