use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take_until, take_while, take_while1},
    character::complete::{char, digit1, multispace1},
    combinator::{cut, map, map_res, opt, recognize, value},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Atom(String),
    Var(String),
    Number(i64),
    Struct {
        functor: String,
        args: Vec<Term>,
    }, // functorは一旦全てInnerStructにパースし、最後にconvert_termする
    List {
        items: Vec<Term>,
        tail: Option<Box<Term>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clause {
    Fact(Term),
    Rule { head: Term, body: Vec<Term> },
}

impl Term {
    pub fn get_name(&self) -> &str {
        match self {
            Term::Var(name) | Term::Atom(name) => name,
            Term::Struct { functor, .. } => functor,
            Term::Number(_) => "<number>",
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
    Term::Var(name.into())
}

#[allow(unused)]
pub(super) fn a(name: impl Into<String>) -> Term {
    Term::Atom(name.into())
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
            |(items, tail)| Term::List {
                items,
                tail: tail.map(Box::new),
            },
        ),
        cut(ws(char(']'))),
    ))
    .parse(input)
}

fn paren_term(input: &str) -> PResult<'_, Term> {
    delimited(ws(char('(')), term, cut(ws(char(')')))).parse(input)
}

fn number_term(input: &str) -> PResult<'_, Term> {
    map(ws(integer), Term::Number).parse(input)
}

fn var_term(input: &str) -> PResult<'_, Term> {
    map(ws(variable), Term::Var).parse(input)
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
            Some(args) => Term::Struct {
                functor: name,
                args,
            },
            None => Term::Atom(name),
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
/// Automatically converts top-level Struct to TopStruct.
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
            Clause::Fact(Term::Struct {
                functor: "parent".to_string(),
                args: vec![a("alice"), a("bob")],
            }),
        );
    }

    #[test]
    fn parse_rule() {
        assert_clause(
            "grandparent(X, Y) :- parent(X, Z), parent(Z, Y).",
            Clause::Rule {
                head: Term::Struct {
                    functor: "grandparent".to_string(),
                    args: vec![v("X"), v("Y")],
                },
                body: vec![
                    Term::Struct {
                        functor: "parent".to_string(),
                        args: vec![v("X"), v("Z")],
                    },
                    Term::Struct {
                        functor: "parent".to_string(),
                        args: vec![v("Z"), v("Y")],
                    },
                ],
            },
        );
    }

    #[test]
    fn parse_list() {
        assert_clause(
            "member(X, [X|_]).",
            Clause::Fact(Term::Struct {
                functor: "member".to_string(),
                args: vec![
                    v("X"),
                    Term::List {
                        items: vec![v("X")],
                        tail: Some(Box::new(v("_"))),
                    },
                ],
            }),
        );
    }

    #[test]
    fn parse_query_simple() {
        let src = "member(X, [1,2,3]).";
        let (_, qs) = query(src).unwrap();
        assert_eq!(
            qs,
            vec![Term::Struct {
                functor: "member".to_string(),
                args: vec![
                    v("X"),
                    Term::List {
                        items: vec![Term::Number(1), Term::Number(2), Term::Number(3),],
                        tail: None,
                    },
                ],
            }]
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
            Clause::Fact(Term::Struct { functor, args }) => {
                assert_eq!(functor, "parent");
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0], Term::Atom(_)));
                match &args[1] {
                    Term::Struct { functor, args } => {
                        assert_eq!(functor, "f");
                        assert_eq!(args.len(), 1);
                        assert!(matches!(args[0], Term::Atom(_)));
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
            Clause::Fact(Term::Atom(name)) => {
                assert_eq!(name, "hello");
            }
            _ => panic!("Expected TopAtom fact, got {:?}", converted),
        }
    }
}
