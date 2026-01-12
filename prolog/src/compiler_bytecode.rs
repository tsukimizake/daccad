use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum WamReg {
    X(usize),
}

impl fmt::Debug for WamReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WamReg::X(n) => write!(f, "X{}", n),
        }
    }
}

#[cfg(debug_assertions)]
pub(crate) type VarName = String;

#[cfg(not(debug_assertions))]
pub(crate) type VarName = ();

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub enum WamInstr {
    // GetStructはまずregが指すレジスタの内容を調べる。
    // 構造体がセットされていなければ、構造体を新たにheap上に作成し、structure_argument_pointerをセットしwrite modeに入る。
    // 構造体がセットされていれば、その構造体のfunctorとarityが合っているか確認し、structure_argument_pointerをセットしread modeに入る。
    // どちらにしろ後続命令はheap上をarity回辿る。
    GetStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },

    GetNumber {
        val: i64,
        reg: WamReg,
    },

    // 初回出現変数の場合はGetVarでregに変数をセットする。ufにも登録する
    GetVar {
        name: VarName,
        reg: WamReg,
    },
    // 2回目以降の出現変数の場合はGetValでregをufに登録してwithとunifyする。
    GetVal {
        name: VarName,
        with: WamReg,
        reg: WamReg,
    },

    // Query, rule bodyで使用。
    // 構造体を新たにheap上に作成し、regにセットする。
    PutStruct {
        functor: String,
        arity: usize,
        reg: WamReg,
    },

    // Query, rule bodyで使用。
    // 初回出現変数の場合はPutVarでregに変数をセット
    PutVar {
        name: VarName,
        argreg: WamReg,
        reg2: WamReg,
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

    // GetStructの後に続く。
    // read modeならheap上の構造体の引数とregをunifyする。
    // write modeならheap上に構造体の引数をregにセットする。
    // structure_argument_pointerを1つ進める。
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

/// Vec<WamInstr>を改行区切りで表示するラッパー
#[allow(dead_code)]
pub struct WamInstrs<'a>(pub &'a [WamInstr]);

impl<'a> fmt::Debug for WamInstrs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for instr in self.0 {
            writeln!(f, "{:?}", instr)?;
        }
        Ok(())
    }
}
