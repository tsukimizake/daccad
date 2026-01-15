use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compile_query::CompiledQuery;
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::{GlobalParentIndex, LayeredUf, Parent};
use crate::parse::{Term, TermId};
use std::collections::HashMap;

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
        Cell::Empty => {
            panic!("getstruct_cell_ref: cell is Empty");
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

            WamInstr::SetVar { name, reg } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set_register(reg, Register::UfRef { id: uf_id });
            }

            WamInstr::SetVal { reg, .. } => {
                if let Register::UfRef { id: prev_id } = registers.get_register(reg) {
                    layered_uf.alloc_node_with_parent(prev_id);
                } else {
                    panic!("SetVal: register does not contain UfRef");
                }
            }

            WamInstr::PutVar {
                name, argreg: reg, ..
            } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set_register(reg, Register::UfRef { id: uf_id });
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

            WamInstr::UnifyVar { name, reg } => {
                match read_write_mode {
                    ReadWriteMode::Read => {
                        // unify
                        todo!("unify read var: {}, {:?}", name, reg);
                    }
                    ReadWriteMode::Write => {
                        // set
                        let id = heap.insert_var(name.clone());
                        registers.set_register(reg, Register::CellRef { id });
                    }
                }
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

pub fn execute_instructions(query: CompiledQuery, orig_query: Vec<Term>) -> Result<Vec<Term>, ()> {
    let mut program_counter = 0;
    let mut exec_mode = ExecMode::Continue;
    let mut layered_uf = LayeredUf::new();
    let mut cell_heap = CellHeap::new();
    let mut registers = Registers::new();

    while exec_mode == ExecMode::Continue {
        exectute_impl(
            &query.instructions,
            &mut program_counter,
            &mut registers,
            &mut cell_heap,
            &mut layered_uf,
            &mut exec_mode,
            &mut ReadWriteMode::Read,
        );
        program_counter += 1;
    }

    if exec_mode == ExecMode::ResolvedToFalse {
        return Err(());
    }

    // orig_queryを走査して変数を解決済みの値で置き換える
    let resolved = orig_query
        .iter()
        .map(|term| {
            resolve_term(
                term,
                &query.term_to_reg,
                &mut registers,
                &cell_heap,
                &mut layered_uf,
            )
        })
        .collect();
    Ok(resolved)
}

/// Termを走査して変数を解決済みの値で置き換える
fn resolve_term(
    term: &Term,
    term_to_reg: &HashMap<TermId, WamReg>,
    registers: &mut Registers,
    heap: &CellHeap,
    uf: &mut LayeredUf,
) -> Term {
    match term {
        Term::Var { id, name, .. } => {
            if let Some(reg) = term_to_reg.get(id) {
                match registers.get_register(reg) {
                    Register::UfRef { id: uf_id } => {
                        let root = uf.find_root(uf_id);
                        cell_to_term(root.cell, uf_id, heap, uf)
                    }
                    Register::CellRef { id: cell_id } => {
                        let cell = heap.value(cell_id);
                        match cell.as_ref() {
                            Cell::Var { name } => Term::new_var(name.clone()),
                            Cell::Struct { functor, arity } => {
                                // CellRefの場合はUfRefではないのでargsの取得が難しい
                                // 現状ではarity=0の場合のみ対応
                                if *arity == 0 {
                                    Term::new_struct(functor.clone(), vec![])
                                } else {
                                    // TODO: ネストした構造体の対応
                                    Term::new_var(name.clone())
                                }
                            }
                            Cell::Empty => Term::new_var(name.clone()),
                        }
                    }
                }
            } else {
                // term_to_regに登録されていない変数はそのまま
                Term::new_var(name.clone())
            }
        }
        Term::Struct { functor, args, .. } => {
            let resolved_args = args
                .iter()
                .map(|arg| resolve_term(arg, term_to_reg, registers, heap, uf))
                .collect();
            Term::new_struct(functor.clone(), resolved_args)
        }
        Term::Number { value, .. } => Term::new_number(*value),
        Term::List { items, tail, .. } => {
            let resolved_items = items
                .iter()
                .map(|item| resolve_term(item, term_to_reg, registers, heap, uf))
                .collect();
            let resolved_tail = tail
                .as_ref()
                .map(|t| Box::new(resolve_term(t, term_to_reg, registers, heap, uf)));
            Term::new_list(resolved_items, resolved_tail)
        }
    }
}

/// CellからTermを再構築する
/// uf_idは構造体の場合に引数を取得するために使う
fn cell_to_term(
    cell_id: CellIndex,
    uf_id: GlobalParentIndex,
    heap: &CellHeap,
    uf: &mut LayeredUf,
) -> Term {
    let cell = heap.value(cell_id);
    match cell.as_ref() {
        Cell::Var { name } => Term::new_var(name.clone()),
        Cell::Struct { functor, arity } => {
            if *arity == 0 {
                Term::new_struct(functor.clone(), vec![])
            } else {
                // 構造体の引数はuf_id + 1からarity個連続している
                let args = (1..=*arity)
                    .map(|i| {
                        let arg_uf_id = GlobalParentIndex::offset(uf_id, i);
                        let arg_root = uf.find_root(arg_uf_id);
                        cell_to_term(arg_root.cell, arg_uf_id, heap, uf)
                    })
                    .collect();
                Term::new_struct(functor.clone(), args)
            }
        }
        Cell::Empty => Term::new_var("_".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile_db::compile_db,
        compile_link::compile_link,
        compile_query::compile_query,
        compiler_bytecode::WamInstrs,
        parse::{self, Term},
    };

    fn compile_program(db_src: &str, query_src: &str) -> (CompiledQuery, Vec<Term>) {
        let db_clauses = parse::database(db_src).expect("failed to parse db");

        let query_terms = parse::query(query_src).expect("failed to parse query").1;
        let linked = compile_link(
            compile_query(query_terms.clone()),
            compile_db(db_clauses.clone()),
        );
        println!("{:?}", WamInstrs(&linked.instructions));
        (linked, query_terms)
    }

    #[test]
    fn simple_atom_match() {
        let (query, query_term) = compile_program("hello.", "hello.");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }
    #[test]
    fn fail_unmatched() {
        let (query, query_term) = compile_program("hello.", "bye.");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn db_var_matches_constant_query() {
        let (query, query_term) = compile_program("honi(X).", "honi(fuwa).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn query_var_binds_to_constant_fact() {
        let (query, query_term) = compile_program("honi(fuwa).", "honi(X).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "honi".to_string(),
            vec![Term::new_struct("fuwa".to_string(), vec![])],
        )];
        assert_eq!(result, Ok(expected));
    }
}
