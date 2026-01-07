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
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TermId(pub(crate) u64);

static NEXT_TERM_ID: AtomicU64 = AtomicU64::new(0);

fn next_term_id() -> TermId {
    TermId(NEXT_TERM_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone)]
pub enum Term {
    Var {
        id: TermId,
        name: String,
    },
    Number {
        id: TermId,
        value: i64,
    },
    Struct {
        id: TermId,
        functor: String,
        args: Vec<Term>,
    },
    List {
        id: TermId,
        items: Vec<Term>,
        tail: Option<Box<Term>>,
    },
}

impl PartialEq for Term {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Term::Var { name: left, .. }, Term::Var { name: right, .. }) => left == right,
            (Term::Number { value: left, .. }, Term::Number { value: right, .. }) => left == right,
            (
                Term::Struct {
                    functor: left_functor,
                    args: left_args,
                    ..
                },
                Term::Struct {
                    functor: right_functor,
                    args: right_args,
                    ..
                },
            ) => left_functor == right_functor && left_args == right_args,
            (
                Term::List {
                    items: left_items,
                    tail: left_tail,
                    ..
                },
                Term::List {
                    items: right_items,
                    tail: right_tail,
                    ..
                },
            ) => left_items == right_items && left_tail == right_tail,
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Clause {
    Fact(Term),
    Rule { head: Term, body: Vec<Term> },
}

impl fmt::Debug for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var { id, name } => write!(f, "{}@{}", name, id.0),
            Term::Number { id, value } => write!(f, "{}@{}", value, id.0),
            Term::Struct {
                id,
                functor,
                args,
            } => {
                write!(f, "{}@{}", functor, id.0)?;
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
            Term::List { id, items, tail } => {
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
                write!(f, "]@{}", id.0)
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

impl Term {
    pub fn new_var(name: String) -> Self {
        Term::Var {
            id: next_term_id(),
            name,
        }
    }

    pub fn new_number(value: i64) -> Self {
        Term::Number {
            id: next_term_id(),
            value,
        }
    }

    pub fn new_struct(functor: String, args: Vec<Term>) -> Self {
        Term::Struct {
            id: next_term_id(),
            functor,
            args,
        }
    }

    pub fn new_list(items: Vec<Term>, tail: Option<Box<Term>>) -> Self {
        Term::List {
            id: next_term_id(),
            items,
            tail,
        }
    }

    pub fn id(&self) -> TermId {
        match self {
            Term::Var { id, .. }
            | Term::Number { id, .. }
            | Term::Struct { id, .. }
            | Term::List { id, .. } => *id,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            Term::Var { name, .. } => name,
            Term::Struct { functor, .. } => functor,
            Term::Number { .. } => "<number>",
            Term::List { .. } => "<list>",
        }
    }
}

impl Clause {
    pub fn mark_top_level_structs(self) -> Self {
        match self {
            Clause::Fact(term) => Clause::Fact(term),
            Clause::Rule { head, body } => Clause::Rule {
                head: head,
                body: body,
            },
        }
    }
}

#[allow(unused)]
pub(super) fn v(name: impl Into<String>) -> Term {
    Term::new_var(name.into())
}

#[allow(unused)]
pub(super) fn a(name: impl Into<String>) -> Term {
    Term::new_struct(name.into(), vec![])
}

pub(super) type PResult<'a, T> = IResult<&'a str, T>;

// Whitespace and comments
fn line_comment(input: &str) -> PResult<'_, ()> {
    // % ... end-of-line
    map(pair(tag("%"), opt(is_not("\n"))), |_| ()).parse(input)
}

fn block_comment(input: &str) -> PResult<'_, ()> {
    // /* ... */ (non-nested)
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
    // '...'
    let escaped_char = preceded(
        char('\\'),
        alt((
            value('\'', char('\'')),
            value('\\', char('\\')),
            // simple escapes for newline and tab
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
            |(items, tail)| Term::new_list(items, tail.map(Box::new)),
        ),
        cut(ws(char(']'))),
    ))
    .parse(input)
}

fn paren_term(input: &str) -> PResult<'_, Term> {
    delimited(ws(char('(')), term, cut(ws(char(')')))).parse(input)
}

fn number_term(input: &str) -> PResult<'_, Term> {
    map(ws(integer), Term::new_number).parse(input)
}

fn var_term(input: &str) -> PResult<'_, Term> {
    map(ws(variable), Term::new_var).parse(input)
}

fn atom_term(input: &str) -> PResult<'_, Term> {
    // structure or plain atom
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
            Some(args) => Term::new_struct(name, args),
            None => Term::new_struct(name, vec![]),
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

pub(super) fn clause(input: &str) -> PResult<'_, Clause> {
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
    ws(terminated(many0(clause), opt(space_or_comment1))).parse(input)
}

/// Parse an entire Prolog database (sequence of clauses) and ensure full consumption.
/// Returns the list of clauses or the first parse error.
/// Applies `mark_top_level_structs` to each clause.
pub fn database(input: &str) -> Result<Vec<Clause>, nom::Err<nom::error::Error<&str>>> {
    match program(input) {
        Ok((rest, clauses)) if rest.is_empty() => Ok(clauses
            .into_iter()
            .map(|clause| clause.mark_top_level_structs())
            .collect()),
        Ok((rest, _)) => Err(nom::Err::Error(nom::error::Error {
            input: rest,
            code: nom::error::ErrorKind::Fail,
        })),
        Err(e) => Err(e),
    }
}

pub fn query(input: &str) -> PResult<'_, Vec<Term>> {
    map(ws(terminated(goals, cut(ws(char('.'))))), |terms| {
        terms.into_iter().map(|term| term).collect()
    })
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{Clause, Term, a, v};

    fn assert_clause(src: &str, expected: Clause) {
        let (_, parsed) = clause(src).unwrap();
        assert_eq!(parsed.mark_top_level_structs(), expected);
    }

    #[test]
    fn parse_fact() {
        assert_clause(
            "parent(alice, bob).",
            Clause::Fact(Term::new_struct(
                "parent".to_string(),
                vec![a("alice"), a("bob")],
            )),
        );
    }

    #[test]
    fn parse_rule() {
        assert_clause(
            "grandparent(X, Y) :- parent(X, Z), parent(Z, Y).",
            Clause::Rule {
                head: Term::new_struct("grandparent".to_string(), vec![v("X"), v("Y")]),
                body: vec![
                    Term::new_struct("parent".to_string(), vec![v("X"), v("Z")]),
                    Term::new_struct("parent".to_string(), vec![v("Z"), v("Y")]),
                ],
            },
        );
    }

    #[test]
    fn parse_list() {
        assert_clause(
            "member(X, [X|_]).",
            Clause::Fact(Term::new_struct(
                "member".to_string(),
                vec![v("X"), Term::new_list(vec![v("X")], Some(Box::new(v("_"))))],
            )),
        );
    }

    #[test]
    fn parse_query_simple() {
        let src = "member(X, [1,2,3]).";
        let (_, qs) = query(src).unwrap();
        assert_eq!(
            qs,
            vec![Term::new_struct(
                "member".to_string(),
                vec![
                    v("X"),
                    Term::new_list(
                        vec![
                            Term::new_number(1),
                            Term::new_number(2),
                            Term::new_number(3),
                        ],
                        None,
                    ),
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
    fn test_top_struct_vs_struct() {
        let src = "parent(alice, f(nested)).";
        let (_, clause) = clause(src).unwrap();
        let converted = clause.mark_top_level_structs();

        match converted {
            Clause::Fact(Term::Struct { functor, args, .. }) => {
                assert_eq!(functor, "parent");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Term::Struct { args, .. } if args.is_empty()));
                match &args[1] {
                    Term::Struct { functor, args, .. } => {
                        assert_eq!(functor, "f");
                        assert_eq!(args.len(), 1);
                        assert!(matches!(&args[0], Term::Struct { args, .. } if args.is_empty()));
                    }
                    _ => panic!("Expected nested Struct, got {:?}", args[1]),
                }
            }
            _ => panic!("Expected TopStruct fact, got {:?}", converted),
        }
    }

    #[test]
    fn test_top_atom() {
        let src = "hello.";
        let (_, clause) = clause(src).unwrap();
        let converted = clause.mark_top_level_structs();

        match converted {
            Clause::Fact(Term::Struct { functor, args, .. }) => {
                assert_eq!(functor, "hello");
                assert_eq!(args.len(), 0);
            }
            _ => panic!("Expected Struct with arity 0 fact, got {:?}", converted),
        }
    }
}
