use crate::cell_heap::{CellHeap, CellIndex};
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::LayeredUf;
use crate::parse::Term;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    registers: Vec<CellIndex>,
}

impl Registers {
    fn new(heap: &mut CellHeap) -> Self {
        let mut registers = Vec::with_capacity(32);
        for _ in 0..32 {
            registers.push(heap.insert_empty());
        }
        Self { registers }
    }

    pub fn get_arg_registers(&self) -> &[CellIndex] {
        &self.registers
    }
    pub fn get_register(&mut self, cell_store: &mut CellHeap, reg: &WamReg) -> CellIndex {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        while index >= self.registers.len() {
            self.registers.push(cell_store.insert_empty());
        }
        self.registers[index]
    }

    pub fn set_register(&mut self, cell_heap: &mut CellHeap, reg: &WamReg, value: CellIndex) {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        while index >= self.registers.len() {
            self.registers.push(cell_heap.insert_empty());
        }
        self.registers[index] = value;
    }
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
    heap: &mut CellHeap,
    layered_uf: &mut LayeredUf,
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
                // stack.push(Frame::Environment {
                //     return_pc: *program_counter,
                //     registers: Vec::new(), // TODO
                // });
                *program_counter = *to_program_counter;
            }
            WamInstr::Label { name: _, arity: _ } => {}
            WamInstr::Proceed => {
                // stack.pop();
                // if stack.len() == 0 {
                *exec_mode = ExecMode::ResolvedToTrue;
                // }
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
    let mut layered_uf = LayeredUf::new();
    let mut cell_heap = CellHeap::new();
    let mut registers = Registers::new(&mut cell_heap);

    while exec_mode == ExecMode::Continue {
        exectute_impl(
            &instructions,
            &mut program_counter,
            &mut registers,
            &mut cell_heap,
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
