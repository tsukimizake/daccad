use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamRegister {
    A(u64), // Argument register
    X(u64), // Temporary register
}

pub type Subst = HashMap<String, Term>;

#[derive(Debug, Clone, PartialEq)]
pub enum WamInstr {
    GetStruct {
        functor: String,
        arity: u64,
        reg: WamRegister,
    },
    GetAtom {
        name: String,
        reg: WamRegister,
    },
    GetNumber {
        val: i64,
        reg: WamRegister,
    },
    GetVar {
        name: String,
        reg: WamRegister,
    },

    PutStruct {
        functor: String,
        arity: u64,
        reg: WamRegister,
    },
    PutVar {
        reg: WamRegister,
        arg: u64,
    },
    PutAtom {
        reg: WamRegister,
        name: u64,
    },
    PutNumber {
        reg: WamRegister,
        val: i64,
    },

    SetStruct {
        name: String,
    },
    SetVar {
        name: String,
    },
    SetAtom {
        name: String,
    },
    SetNumber {
        val: i64,
    },

    UnifyAtom {
        name: String,
    },
    UnifyNumber {
        val: i64,
    },
    UnifyVar {
        name: String,
    },

    Call {
        predicate: String,
        arity: u64,
    },
    Execute {
        predicate: u64,
        arity: u64,
    },
    Allocate {
        size: u64,
    },
    Deallocate,
    Proceed,

    TryMeElse {
        target: u64,
    },
    RetryMeElse {
        target: u64,
    },
    TrustMeElseFail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Ref(Rc<Cell>),
    Str { functor: u64, arity: u64 },
    Atom(String),
    Number(i64),
}

pub enum Frame {
    Environment {
        return_pc: Weak<WamInstr>,
        prev_ep: Weak<Frame>,
        local_start: Weak<Cell>, // ?
    },

    ChoicePoint {
        // TODO
    },
}
