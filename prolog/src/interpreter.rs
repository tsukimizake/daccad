use std::{collections::HashMap, rc::Rc};

use crate::types::{Cell, Frame, WamInstr};

struct Machine {
    heap: Vec<Cell>, // Hレジスタはheap.len()
    stack: Vec<Frame>,
    trail: Vec<Rc<Cell>>,     // 変更された参照セルのヒープ位置
    arg_registers: Vec<Cell>, // TODO runtime_sized_arrayにする可能性
    other_registers: Vec<Cell>,
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
