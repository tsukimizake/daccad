use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until, take_while, take_while1},
    character::complete::{char, digit1, multispace1, one_of},
    combinator::{cut, map, map_opt, map_res, opt, recognize, value},
    error::{context, VerboseError},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};

pub type PResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Var(String),
    Atom(String),
    Number(i64),
    Struct { functor: String, args: Vec<Term> },
    List { items: Vec<Term>, tail: Option<Box<Term>> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clause {
    Fact(Term),
    Rule { head: Term, body: Vec<Term> },
}

// Whitespace and comments
fn line_comment(input: &str) -> PResult<'_, ()> {
    // % ... end-of-line
    map(
        pair(tag("%"), opt(is_not("\n"))),
        |_| (),
    )(input)
}

fn block_comment(input: &str) -> PResult<'_, ()> {
    // /* ... */ (non-nested)
    map(
        tuple((tag("/*"), cut(take_until("*/")), tag("*/"))),
        |_| (),
    )(input)
}

fn space_or_comment1(input: &str) -> PResult<'_, ()> {
    value((), many0(alt((
        value((), multispace1),
        line_comment,
        block_comment,
    ))))(input)
}

fn space_or_comment0(input: &str) -> PResult<'_, ()> {
    value((), many0(alt((
        value((), multispace1),
        line_comment,
        block_comment,
    ))))(input)
}

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> PResult<'a, O>
where
    F: Fn(&'a str) -> PResult<'a, O>,
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
        recognize(pair(
            take_while1(is_atom_start),
            take_while(is_id_continue),
        )),
        |s: &str| s.to_string(),
    )(input)
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
    )(input)
}

fn atom(input: &str) -> PResult<'_, String> {
    alt((quoted_atom, unquoted_atom))(input)
}

fn variable(input: &str) -> PResult<'_, String> {
    map(
        recognize(pair(
            take_while1(is_var_start),
            take_while(is_id_continue),
        )),
        |s: &str| s.to_string(),
    )(input)
}

fn integer(input: &str) -> PResult<'_, i64> {
    map_res(
        recognize(pair(opt(char('-')), digit1)),
        |s: &str| s.parse::<i64>(),
    )(input)
}

// Terms
fn list_term(input: &str) -> PResult<'_, Term> {
    context(
        "list",
        ws(delimited(
            char('['),
            map(
                tuple((
                    separated_list0(ws(char(',')), term),
                    opt(preceded(ws(char('|')), term)),
                )),
                |(items, tail)| Term::List {
                    items,
                    tail: tail.map(Box::new),
                },
            ),
            cut(ws(char(']'))),
        )),
    )(input)
}

fn paren_term(input: &str) -> PResult<'_, Term> {
    delimited(ws(char('(')), term, cut(ws(char(')'))))(input)
}

fn number_term(input: &str) -> PResult<'_, Term> {
    map(ws(integer), Term::Number)(input)
}

fn var_term(input: &str) -> PResult<'_, Term> {
    map(ws(variable), Term::Var)(input)
}

fn atom_term(input: &str) -> PResult<'_, Term> {
    // structure or plain atom
    context(
        "atom_or_struct",
        ws(map(
            pair(atom, opt(ws(delimited(
                char('('),
                separated_list0(ws(char(',')), term),
                cut(ws(char(')'))),
            )))),
            |(name, maybe_args)| match maybe_args {
                Some(args) => Term::Struct {
                    functor: name,
                    args,
                },
                None => Term::Atom(name),
            },
        )),
    )(input)
}

fn simple_term(input: &str) -> PResult<'_, Term> {
    alt((list_term, paren_term, number_term, var_term, atom_term))(input)
}

pub fn term(input: &str) -> PResult<'_, Term> {
    simple_term(input)
}

fn goals(input: &str) -> PResult<'_, Vec<Term>> {
    separated_list1(ws(char(',')), term)(input)
}

pub fn clause(input: &str) -> PResult<'_, Clause> {
    context(
        "clause",
        ws(terminated(
            alt((
                map(
                    separated_pair(term, ws(tag(":-")), goals),
                    |(head, body)| Clause::Rule { head, body },
                ),
                map(term, Clause::Fact),
            )),
            cut(ws(char('.'))),
        )),
    )(input)
}

pub fn program(input: &str) -> PResult<'_, Vec<Clause>> {
    ws(terminated(many0(clause), opt(space_or_comment1)))(input)
}

pub fn query(input: &str) -> PResult<'_, Vec<Term>> {
    context(
        "query",
        ws(terminated(preceded(ws(tag("?-")), goals), cut(ws(char('.'))))),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fact() {
        let src = "parent(alice, bob).";
        let (_, c) = clause(src).unwrap();
        match c {
            Clause::Fact(Term::Struct { functor, args }) => {
                assert_eq!(functor, "parent");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected fact"),
        }
    }

    #[test]
    fn parse_rule() {
        let src = "grandparent(X, Y) :- parent(X, Z), parent(Z, Y).";
        let (_, c) = clause(src).unwrap();
        match c {
            Clause::Rule { head, body } => {
                match head {
                    Term::Struct { functor, args } => {
                        assert_eq!(functor, "grandparent");
                        assert_eq!(args.len(), 2);
                    }
                    _ => panic!("expected struct head"),
                }
                assert_eq!(body.len(), 2);
            }
            _ => panic!("expected rule"),
        }
    }

    #[test]
    fn parse_list() {
        let src = "member(X, [X|_]).";
        let (_, c) = clause(src).unwrap();
        match c {
            Clause::Fact(Term::Struct { functor, args }) => {
                assert_eq!(functor, "member");
                assert_eq!(args.len(), 2);
                match &args[1] {
                    Term::List { items, tail } => {
                        assert_eq!(items.len(), 1);
                        assert!(tail.is_some());
                    }
                    _ => panic!("expected list"),
                }
            }
            _ => panic!("expected fact"),
        }
    }

    #[test]
    fn parse_query_simple() {
        let src = "?- member(X, [1,2,3]).";
        let (_, qs) = query(src).unwrap();
        assert_eq!(qs.len(), 1);
    }
}
