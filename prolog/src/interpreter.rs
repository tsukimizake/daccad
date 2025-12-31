use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::{GlobalParentIndex, LayeredUf, Parent};
use crate::parse::Term;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Register {
    CellRef { id: CellIndex },
    UfRef { id: GlobalParentIndex },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Registers {
    registers: Vec<Register>,
}

impl Registers {
    fn new() -> Self {
        let mut registers = Vec::with_capacity(32);
        for _ in 0..32 {
            registers.push(Register::CellRef {
                id: CellIndex::EMPTY,
            });
        }
        Self { registers }
    }

    pub fn get_arg_registers(&self) -> &[Register] {
        &self.registers
    }
    pub fn get_register(&mut self, reg: &WamReg) -> Register {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        while index >= self.registers.len() {
            self.registers.push(Register::CellRef {
                id: CellIndex::EMPTY,
            });
        }
        self.registers[index].clone()
    }

    pub fn set_register(&mut self, reg: &WamReg, value: Register) {
        let index = match reg {
            WamReg::X(index) => *index,
        };

        while index >= self.registers.len() {
            self.registers.push(Register::CellRef {
                id: CellIndex::EMPTY,
            });
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

#[derive(PartialEq, Eq, Debug)]
enum ReadWriteMode {
    Read,
    Write,
}

fn getstruct_cell_ref(
    cell: &Cell,
    reg: &WamReg,
    functor: &String,
    arity: &usize,
    registers: &mut Registers,
    heap: &mut CellHeap,
    read_write_mode: &mut ReadWriteMode,
    exec_mode: &mut ExecMode,
) {
    match cell {
        Cell::Empty => {
            // panicが正しいかもしれない
            *read_write_mode = ReadWriteMode::Write;
        }
        Cell::Var { .. } => {
            *read_write_mode = ReadWriteMode::Write;
        }
        Cell::Struct {
            functor: existing_functor,
            arity: existing_arity,
        } => {
            if existing_functor == functor && existing_arity == arity {
                let cell_id = heap.insert_struct(functor, *arity);
                registers.set_register(reg, Register::CellRef { id: cell_id });

                *read_write_mode = ReadWriteMode::Read;
            } else {
                *exec_mode = ExecMode::ResolvedToFalse;
            }
        }
    }
}

fn exectute_impl(
    instructions: &[WamInstr],
    program_counter: &mut usize,
    registers: &mut Registers,
    heap: &mut CellHeap,
    layered_uf: &mut LayeredUf,
    exec_mode: &mut ExecMode,
    read_write_mode: &mut ReadWriteMode,
) {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutStruct {
                functor,
                arity,
                reg,
            } => {
                let id = heap.insert_struct(functor, *arity);
                registers.set_register(reg, Register::CellRef { id });
            }

            WamInstr::SetVar { name: _, reg } => {}
            WamInstr::SetVal { name: _, reg } => {}

            WamInstr::PutVar { name, reg, .. } => {
                let id = heap.insert_var(name);
                registers.set_register(reg, Register::CellRef { id });
            }

            WamInstr::GetStruct {
                functor,
                arity,
                reg,
            } => match registers.get_register(reg) {
                Register::CellRef { id } => {
                    let cell = heap.value(id);
                    getstruct_cell_ref(
                        cell.as_ref(),
                        reg,
                        functor,
                        arity,
                        registers,
                        heap,
                        read_write_mode,
                        exec_mode,
                    );
                }
                Register::UfRef { id } => {
                    let Parent { cell, .. } = layered_uf.find_root(id);
                    let cell = heap.value(*cell);
                    getstruct_cell_ref(
                        cell.as_ref(),
                        reg,
                        functor,
                        arity,
                        registers,
                        heap,
                        read_write_mode,
                        exec_mode,
                    );
                }
            },

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

pub fn execute_instructions(
    instructions: Vec<WamInstr>,
    orig_query: Vec<Term>,
) -> Result<Vec<Term>, ()> {
    let mut program_counter = 0;
    let mut exec_mode = ExecMode::Continue;
    let mut layered_uf = LayeredUf::new();
    let mut cell_heap = CellHeap::new();
    let mut registers = Registers::new();

    while exec_mode == ExecMode::Continue {
        exectute_impl(
            &instructions,
            &mut program_counter,
            &mut registers,
            &mut cell_heap,
            &mut layered_uf,
            &mut exec_mode,
            &mut ReadWriteMode::Read,
        );
        program_counter += 1;
    }
    println!("{:?}", instructions);
    println!("{:?}", orig_query);

    if exec_mode == ExecMode::ResolvedToFalse {
        return Err(());
    } else {
        Ok(orig_query) // TODO return query with bindings applied
    }
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

    fn compile_program(db_src: &str, query_src: &str) -> (Vec<WamInstr>, Vec<Term>) {
        let db_clauses = parse::database(db_src).expect("failed to parse db");

        println!("DB Clauses: {:#?}", db_clauses);
        let query_terms = parse::query(query_src).expect("failed to parse query").1;
        let instructions = compile_link(
            compile_query(query_terms.clone()),
            compile_db(db_clauses.clone()),
        );
        println!("Compiled Instructions: {:#?}", compile_db(db_clauses));
        (instructions, query_terms)
    }

    #[test]
    fn simple_atom_match() {
        let (instructions, query_term) = compile_program("hello.", "hello.");
        let result = execute_instructions(instructions, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }
    #[test]
    fn fail_unmatched() {
        let (instructions, query_term) = compile_program("hello.", "bye.");
        let result = execute_instructions(instructions, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn db_var_matches_constant_query() {
        let (instructions, query_term) = compile_program("honi(X).", "honi(fuwa).");
        let result = execute_instructions(instructions, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn query_var_binds_to_constant_fact() {
        let (instructions, query_term) = compile_program("honi(fuwa).", "honi(X).");
        let result = execute_instructions(instructions, query_term);
        let expected = vec![Term::new_struct(
            "honi".to_string(),
            vec![Term::new_struct("fuwa".to_string(), vec![])],
        )];
        assert_eq!(result, Ok(expected));
    }
}
