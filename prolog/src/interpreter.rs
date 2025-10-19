use std::{collections::HashMap, rc::Rc};

use crate::compiler_bytecode::WamInstr;

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum HeapCell {
    Ref(Rc<HeapCell>),
    Struct { functor: u32, arity: usize },
    Atom(String),
    Number(i64),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum RegStackCell {
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

#[allow(unused)]
enum Frame {
    Base {},
    Environment {
        prev_ep: Rc<Frame>,
        return_pc: Rc<WamInstr>,
        registers: Vec<RegStackCell>,
    },

    ChoicePoint {
        // TODO
    },
}

#[allow(unused)]
struct Machine<'a> {
    heap: Vec<RegStackCell>, // Hレジスタはheap.len()
    stack: Vec<Rc<Frame>>,
    trail: Vec<Rc<RegStackCell>>,     // 変更された参照セルのヒープ位置
    arg_registers: Vec<RegStackCell>, // TODO runtime_sized_arrayにする可能性
    other_registers: Vec<RegStackCell>,
    program: &'a [WamInstr],
    pc: &'a WamInstr,
    env_p: Rc<Frame>,    // 現在の環境フレーム先頭
    choice_p: Rc<Frame>, // 現在の選択ポイントフレーム先頭
}

#[allow(unused)]
impl<'a> Machine<'a> {
    fn new(program: &'a [WamInstr]) -> Self {
        let stack_head = Rc::new(Frame::Base {});
        let stack = vec![stack_head.clone()];
        Self {
            heap: Vec::new(),
            stack: stack,
            trail: Vec::new(),
            arg_registers: Vec::new(),
            other_registers: Vec::new(),
            program,
            pc: &program[0],
            env_p: stack_head.clone(),
            choice_p: stack_head,
        }
    }
}
