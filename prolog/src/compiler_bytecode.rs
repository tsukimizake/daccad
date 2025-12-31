#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamReg {
    X(usize),
}

#[cfg(debug_assertions)]
pub(crate) type VarName = String;

#[cfg(not(debug_assertions))]
pub(crate) type VarName = ();

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub enum WamInstr {
    GetStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },

    GetNumber {
        val: i64,
        reg: WamReg,
    },
    GetVar {
        name: VarName,
        reg: WamReg,
    },
    GetVal {
        name: VarName,
        with: WamReg,
        reg: WamReg,
    },

    PutStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },
    PutVar {
        name: VarName,
        reg: WamReg, // TODO Xレジスタの宣言のためにVec？必要な場合がわかってない
    },
    PutNumber {
        val: i64,
        reg: WamReg,
    },

    SetVar {
        name: VarName,
        reg: WamReg,
    },
    SetVal {
        name: VarName,
        reg: WamReg,
    },
    SetNumber {
        val: i64,
    },

    UnifyVar {
        name: VarName,
        reg: WamReg,
    },

    UnifyVal {
        name: VarName,
        reg: WamReg,
    },

    CallTemp {
        predicate: String,
        arity: usize,
    },
    Call {
        predicate: String,
        arity: usize,
        to_program_counter: usize,
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
    Label {
        name: String,
        arity: usize,
    },
    Error {
        message: String,
    },
}
