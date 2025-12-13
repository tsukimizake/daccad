use crate::{layered_uf::LayeredUf, parse::Term};
use std::rc::Rc;

use crate::compiler_bytecode::{WamInstr, WamReg};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Cell {
    Empty,
    Var {
        name: String,
    },
    Struct {
        functor: String,
        arity: usize,
        children: Vec<Rc<Cell>>,
    },
    Number(i64),
}

// (register/stackdepth, register index)
// registers上なら第一引数はusize::MAX
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
struct CellIndex(usize, usize);

impl CellIndex {
    fn get_from_register(reg_index: usize) -> CellIndex {
        CellIndex(usize::max_value(), reg_index)
    }

    fn get_from_stack(stack_depth: usize, reg_index: usize) -> CellIndex {
        CellIndex(stack_depth, reg_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    registers: Vec<Cell>,
}

impl Registers {
    fn new() -> Self {
        Self {
            registers: vec![Cell::Empty; 32],
        }
    }

    pub fn get_arg_registers(&self) -> &[Cell] {
        &self.registers
    }
    pub fn get_register<'a>(&'a mut self, reg: &WamReg) -> &'a Cell {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        if index < self.registers.len() {
            &self.registers[index]
        } else {
            let r = Cell::Empty;
            self.registers.push(r);
            &self.registers[index]
        }
    }

    pub fn set_register(&mut self, reg: &WamReg, value: Cell) {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        if index >= self.registers.len() {
            self.registers.resize(index + 1, Cell::Empty);
        }
        self.registers[index] = value;
    }
}

enum Frame {
    Base {},
    Environment {
        return_pc: usize,
        registers: Vec<Cell>,
    },
    ChoicePoint {
        stack_len_to_set: usize,
        layered_uf_depth_to_set: usize,
    },
}

#[derive(PartialEq, Eq, Debug)]
enum ExecMode {
    Continue,
    ResolvedToTrue,
    ResolvedToFalse,
}

fn exectute_impl(
    instructions: &[WamInstr],
    program_counter: &mut usize,
    registers: &mut Registers,
    stack: &mut Vec<Frame>,
    layered_uf: &mut LayeredUf<CellIndex>,
    exec_mode: &mut ExecMode,
) {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutStruct {
                functor,
                arity,
                reg,
            } => {}

            WamInstr::SetVar { reg, name: _ } => {}
            WamInstr::SetVal { reg, name: _ } => {}

            WamInstr::PutVar { name: _, reg } => {}

            WamInstr::GetStruct {
                functor,
                arity,
                reg,
            } => {}
            WamInstr::Call {
                predicate: _,
                arity: _,
                to_program_counter,
            } => {
                stack.push(Frame::Environment {
                    return_pc: *program_counter,
                    registers: Vec::new(), // TODO
                });
                *program_counter = *to_program_counter;
            }
            WamInstr::Label { name: _, arity: _ } => {}
            WamInstr::Proceed => {
                stack.pop();
                if stack.len() == 0 {
                    *exec_mode = ExecMode::ResolvedToTrue;
                }
            }
            WamInstr::Error { message } => {
                println!("{}", message);
                *exec_mode = ExecMode::ResolvedToFalse;
            }

            instr => {
                todo!("{:?}", instr);
            }
        }
    } else {
        todo!("current_instr is None");
    }
}

pub fn execute_instructions(instructions: Vec<WamInstr>, orig_query: Term) -> Term {
    let mut program_counter = 0;
    let mut exec_mode = ExecMode::Continue;
    let mut registers = Registers::new();
    let mut stack = Vec::with_capacity(100);
    stack.push(Frame::Base {});
    let mut layered_uf = LayeredUf::new();

    while exec_mode == ExecMode::Continue {
        exectute_impl(
            &instructions,
            &mut program_counter,
            &mut registers,
            &mut stack,
            &mut layered_uf,
            &mut exec_mode,
        );
        program_counter += 1;
    }

    todo!("return query with bindings applied")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile_db::compile_db,
        compile_link::compile_link,
        compile_query::compile_query,
        parse::{self, Term},
    };

    fn compile_program(db_src: &str, query_src: &str) -> (Vec<WamInstr>, Term) {
        let db_clauses = parse::database(db_src).expect("failed to parse db");
        let query_terms = parse::query(query_src).expect("failed to parse query").1;
        let mut query_terms = query_terms.into_iter();
        let query_term = query_terms.next().expect("query needs at least one term");
        let instructions = compile_link(
            compile_query(vec![query_term.clone()]),
            compile_db(db_clauses),
        );
        (instructions, query_term)
    }

    #[test]
    fn execute_instructions_signature() {
        let _func: fn(Vec<WamInstr>, Term) -> Term = execute_instructions;
    }

    #[test]
    #[ignore = "execute_instructions is not implemented yet"]
    fn compiled_program_is_accepted() {
        let (instructions, query_term) = compile_program("hello.", "hello.");
        let _ = execute_instructions(instructions, query_term);
    }
}
