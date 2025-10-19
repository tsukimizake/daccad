use std::rc::Rc;

use crate::compiler_bytecode::{WamInstr, WamReg};

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    Empty,
    Ref(Rc<Cell>),
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
        registers: Vec<Cell>,
    },

    ChoicePoint {
        // TODO
    },
}

#[allow(unused)]
struct TrailEntry {
    cells_to_revert: Vec<Cell>,
}

#[allow(unused)]
#[derive(Debug, Clone)]
struct RuntimeRegBank {
    registers: Vec<Cell>,
}

impl RuntimeRegBank {
    fn new() -> Self {
        Self {
            registers: vec![Cell::Empty; 32],
        }
    }

    fn get(&self, index: usize) -> &Cell {
        if index < self.registers.len() {
            &self.registers[index]
        } else {
            &Cell::Empty
        }
    }

    fn insert(&mut self, index: usize, value: Cell) {
        if index >= self.registers.len() {
            self.registers.resize(index + 1, Cell::Empty);
        }
        self.registers[index] = value;
    }
}

#[allow(unused)]
struct Machine {
    heap: Vec<Rc<Cell>>, // Hレジスタはheap.len()
    stack: Vec<Rc<Frame>>,
    arg_registers: RuntimeRegBank,
    other_registers: RuntimeRegBank,
    instructions: Vec<WamInstr>,
    counter: usize,
    env_p: Rc<Frame>,    // 現在の環境フレーム先頭
    choice_p: Rc<Frame>, // 現在の選択ポイントフレーム先頭
    trail: Vec<TrailEntry>,
}

#[allow(unused)]
impl Machine {
    pub(super) fn new(instructions: Vec<WamInstr>) -> Self {
        let stack_head = Rc::new(Frame::Base {});
        let stack = vec![stack_head.clone()];
        Self {
            heap: Vec::with_capacity(32),
            stack: stack,
            arg_registers: RuntimeRegBank::new(),
            other_registers: RuntimeRegBank::new(),
            instructions,
            counter: 0,
            env_p: stack_head.clone(),
            choice_p: stack_head,
            trail: Vec::with_capacity(32),
        }
    }
    fn step(&mut self) -> bool {
        if let Some(current_instr) = self.instructions.get(self.counter) {
            match current_instr {
                WamInstr::PutAtom { name, reg } => {
                    let cell = Cell::Atom(name.clone());
                    match reg {
                        crate::compiler_bytecode::WamReg::A(index) => {
                            self.arg_registers.insert(*index, cell);
                            true
                        }
                        crate::compiler_bytecode::WamReg::X(index) => {
                            self.other_registers.insert(*index, cell);
                            true
                        }
                    }
                }
                WamInstr::GetAtom { name, reg } => {
                    let derefed = self.deref_reg(&reg);
                    match derefed {
                        Cell::Empty => {
                            // レジスタが空の場合、アトムを設定
                            let cell = Cell::Atom(name.clone());
                            match reg {
                                crate::compiler_bytecode::WamReg::A(index) => {
                                    self.arg_registers.insert(*index, cell);
                                    true
                                }
                                crate::compiler_bytecode::WamReg::X(index) => {
                                    self.other_registers.insert(*index, cell);
                                    true
                                }
                            }
                        }
                        Cell::Atom(ref existing_name) if existing_name == name => true,
                        _ => false,
                    }
                }
                _ => {
                    todo!();
                }
            }
        } else {
            false
        }
    }

    fn deref_reg(&self, wamreg: &WamReg) -> Cell {
        let reg = match wamreg {
            WamReg::A(index) => self.arg_registers.get(*index).clone(),
            WamReg::X(index) => self.other_registers.get(*index).clone(),
        };

        match reg {
            Cell::Ref(rc_cell) => {
                let deref_cell = rc_cell.as_ref();
                self.deref_cell(deref_cell)
            }
            _ => reg.clone(),
        }
    }
    fn deref_cell(&self, cell: &Cell) -> Cell {
        match cell {
            Cell::Ref(rc_cell) => self.deref_cell(&rc_cell),
            _ => cell.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compile_db;

    use super::*;

    fn test(db_str: String, query_str: String, _expect_todo: ()) {
        let db_clauses = crate::parse::database(&db_str).unwrap();
        let (_, query_terms) = crate::parse::query(&query_str).unwrap();
        let db = compile_db::Compiler::new().compile_db(db_clauses);
        let _query = crate::compile_query::Compiler::new().compile(query_terms);
        let mut machine = Machine::new(db);
        // 以下TODO
        machine.step();
        let expected = Cell::Atom("hello".to_string());
        assert_eq!(machine.arg_registers.get(0), &expected);
    }
    #[test]
    fn test_simple_put_atom() {
        test("hello.".to_string(), "hello.".to_string(), ());
    }

    #[allow(unused)]
    //TODO #[test]
    fn test_socrates() {
        test(
            r#"human(socrates). mortal(X) :- human(X)."#.to_string(),
            "mortal(socrates).".to_string(),
            (),
        );
    }
}
