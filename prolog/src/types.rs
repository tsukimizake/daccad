use std::collections::HashMap;

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

/// Convenience helper for building a variable term in tests and examples.
pub fn v(name: impl Into<String>) -> Term {
    Term::Var(name.into())
}

/// Convenience helper for building an atom term in tests and examples.
pub fn a(name: impl Into<String>) -> Term {
    Term::Atom(name.into())
}

pub type Subst = HashMap<String, Term>;
