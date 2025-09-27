#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Var(String),
    Atom(String),
    Number(i64),
    Struct {
        functor: String,
        args: Vec<Term>,
    },
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
