use std::{collections::HashMap, rc::Weak};

use crate::types::{Cell, Frame, WamInstr};

struct Machine {
    heap: Vec<Cell>, // Hレジスタはheap.len()
    stack: Vec<Frame>,
    trail: Vec<Weak<Cell>>, // 変更された参照セルのヒープ位置
    arg_registers: [Cell; 32],
    other_registers: [Cell; 32],
    program: Vec<WamInstr>,
    pc: usize,
    ep: Weak<Frame>, // 現在の環境フレーム先頭 (E)
    cp: Weak<Frame>, // 現在の選択ポイントフレーム先頭 (B)
    name_table: HashMap<usize, String>,
}

impl Machine {
    pub fn lookup_name(&self, id: usize) -> &String {
        self.name_table.get(&id).unwrap()
    }
}
