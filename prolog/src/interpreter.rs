use std::rc::Rc;

use crate::compiler_bytecode::WamInstr;

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum HeapCell {
    Empty,
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum RegStackCell {
    Empty,
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
struct TrailEntry {
    cells_to_revert: Vec<RegStackCell>,
}

#[allow(unused)]
#[derive(Debug, Clone)]
struct RuntimeRegBank {
    registers: Vec<RegStackCell>,
}

impl RuntimeRegBank {
    fn new() -> Self {
        Self {
            registers: vec![RegStackCell::Empty; 32],
        }
    }

    fn get(&self, index: usize) -> &RegStackCell {
        if index < self.registers.len() {
            &self.registers[index]
        } else {
            &RegStackCell::Empty
        }
    }

    fn insert(&mut self, index: usize, value: RegStackCell) {
        if index >= self.registers.len() {
            self.registers.resize(index + 1, RegStackCell::Empty);
        }
        self.registers[index] = value;
    }

    fn len(&self) -> usize {
        self.registers.len()
    }
}

#[allow(unused)]
struct Machine<'a> {
    heap: Vec<Rc<RegStackCell>>, // Hレジスタはheap.len()
    stack: Vec<Rc<Frame>>,
    arg_registers: RuntimeRegBank,
    other_registers: RuntimeRegBank,
    program: &'a [WamInstr],
    current_instr: &'a WamInstr, // program counter相当
    env_p: Rc<Frame>,            // 現在の環境フレーム先頭
    choice_p: Rc<Frame>,         // 現在の選択ポイントフレーム先頭
    trail: Vec<TrailEntry>,
}

#[allow(unused)]
impl<'a> Machine<'a> {
    pub(super) fn new(program: &'a [WamInstr]) -> Self {
        let stack_head = Rc::new(Frame::Base {});
        let stack = vec![stack_head.clone()];
        Self {
            heap: Vec::with_capacity(32),
            stack: stack,
            arg_registers: RuntimeRegBank::new(),
            other_registers: RuntimeRegBank::new(),
            program,
            current_instr: &program[0],
            env_p: stack_head.clone(),
            choice_p: stack_head,
            trail: Vec::with_capacity(32),
        }
    }
    fn step(&mut self) {
        match self.current_instr {
            WamInstr::PutAtom { name, reg } => {
                let cell = RegStackCell::Atom(name.clone());
                match reg {
                    crate::compiler_bytecode::WamReg::A(index) => {
                        self.arg_registers.insert(*index, cell);
                    }
                    crate::compiler_bytecode::WamReg::X(index) => {
                        self.other_registers.insert(*index, cell);
                    }
                }
            }
            WamInstr::GetAtom { name, reg } => {
                let cell = RegStackCell::Atom(name.clone());
                match reg {
                    crate::compiler_bytecode::WamReg::A(index) => {
                        self.arg_registers.insert(*index, cell);
                    }
                    crate::compiler_bytecode::WamReg::X(index) => {
                        self.other_registers.insert(*index, cell);
                    }
                }
            }
            _ => {
                todo!();
            }
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
        let mut machine = Machine::new(&db);
        // 以下TODO
        machine.step();
        let expected = RegStackCell::Atom("hello".to_string());
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
