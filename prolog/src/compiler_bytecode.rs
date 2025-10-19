use crate::parse::Term;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamReg {
    A(u32), // Argument register
    X(u32), // Temporary register
}

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
        name: String,
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
