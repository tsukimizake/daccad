use std::{collections::HashMap, rc::Rc};

use crate::compiler_bytecode::WamInstr;

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeapCell {
    Ref(Rc<HeapCell>),
    Struct { functor: u32, arity: usize },
    Atom(String),
    Number(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegStackCell {
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

enum Frame {
    Environment {
        prev_ep: Rc<Frame>,
        return_pc: Rc<WamInstr>,
        registers: Vec<RegStackCell>,
    },

    ChoicePoint {
        // TODO
    },
}

struct Machine {
    heap: Vec<RegStackCell>, // Hレジスタはheap.len()
    stack: Vec<Frame>,
    trail: Vec<Rc<RegStackCell>>,     // 変更された参照セルのヒープ位置
    arg_registers: Vec<RegStackCell>, // TODO runtime_sized_arrayにする可能性
    other_registers: Vec<RegStackCell>,
    program: Vec<WamInstr>,
    pc: usize,
    ep: Rc<Frame>, // 現在の環境フレーム先頭 ()
    cp: Rc<Frame>, // 現在の選択ポイントフレーム先頭 (B)
    name_table: HashMap<usize, String>,
}

impl Machine {
    pub fn lookup_name(&self, id: usize) -> &String {
        self.name_table.get(&id).unwrap()
    }
}
