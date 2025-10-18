use std::{collections::HashMap, rc::Rc};

///////////////
// AST
///////////////

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

///////////////
// WAM bitecode
///////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamReg {
    A(u32), // Argument register
    X(u32), // Temporary register
}

pub type Subst = HashMap<String, Term>;

#[derive(Debug, Clone, PartialEq)]
pub enum WamInstr {
    GetStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },
    GetAtom {
        name: String,
        reg: WamReg,
    },
    GetNumber {
        val: i64,
        reg: WamReg,
    },
    GetVar {
        name: String,
        reg: WamReg,
    },

    PutStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },
    PutVar {
        name: String,
        reg: WamReg, // TODO Xレジスタの宣言のためにVec？必要な場合がわかってない
    },
    PutAtom {
        name: u32,
        reg: WamReg,
    },
    PutNumber {
        val: i64,
        reg: WamReg,
    },

    SetVar {
        reg: WamReg,
    },
    SetAtom {
        name: String,
    },
    SetNumber {
        val: i64,
    },

    UnifyAtom {
        reg: WamReg,
    },
    UnifyNumber {
        val: i64,
    },
    UnifyVar {
        reg: WamReg,
    },

    Call {
        predicate: String,
        arity: usize,
    },
    Execute {
        predicate: u32,
        arity: usize,
    },
    Allocate {
        size: u32,
    },
    Deallocate,
    Proceed,

    TryMeElse {
        target: u32,
    },
    RetryMeElse {
        target: u32,
    },
    TrustMeElseFail,
}

///////////////
// WAM interpreter
///////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeapCell {
    Ref(Rc<HeapCell>),
    Struct { functor: u32, arity: usize },
    Atom(String),
    Number(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegStackCell {
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

pub enum Frame {
    Environment {
        prev_ep: Rc<Frame>,
        return_pc: Rc<WamInstr>,
        registers: Vec<RegStackCell>,
    },

    ChoicePoint {
        // TODO
    },
}
