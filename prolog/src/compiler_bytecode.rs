#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamReg {
    X(usize),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub enum WamInstr {
    GetStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },

    // TODO Atom系はobsolete getstructure/0にまとめる
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
        name: String,
        reg: WamReg,
    },
    SetVal {
        name: String,
        reg: WamReg,
    },
    SetAtom {
        name: String,
    },
    SetNumber {
        val: i64,
    },

    UnifyVar {
        name: String,
        reg: WamReg,
    },

    UnifyVal {
        name: String,
        reg: WamReg,
    },

    CallTemp {
        predicate: String,
        arity: usize,
    },
    Call {
        predicate: String,
        arity: usize,
        to_linum: usize,
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
