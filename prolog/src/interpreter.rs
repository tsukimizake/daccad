use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compile_query::CompiledQuery;
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::{GlobalParentIndex, LayeredUf, Parent};
use crate::parse::Term;
use crate::resolve_term::resolve_term;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registers {
    registers: Vec<GlobalParentIndex>,
}

impl Registers {
    fn new() -> Self {
        Self {
            registers: vec![GlobalParentIndex::EMPTY; 32],
        }
    }

    #[inline]
    pub(crate) fn get_register(&self, reg: &WamReg) -> GlobalParentIndex {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        self.registers
            .get(index)
            .copied()
            .unwrap_or(GlobalParentIndex::EMPTY)
    }

    #[inline]
    pub fn set_register(&mut self, reg: &WamReg, value: GlobalParentIndex) {
        let index = match reg {
            WamReg::X(index) => *index,
        };

        if index >= self.registers.len() {
            self.registers.resize(index + 1, GlobalParentIndex::EMPTY);
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

// Readが持っているのは構造体の親Indexを起点としたカーソル。WAMでいうSレジスタの値
// 子要素までRead/Writeし終わった後はそのままの値で放置され、次回のGetStructで再設定される。
#[derive(PartialEq, Eq, Debug)]
enum ReadWriteMode {
    Read(GlobalParentIndex),
    Write,
}

fn getstruct_cell_ref(
    existing_parent_index: GlobalParentIndex,
    existing_cell: CellIndex,
    op_reg: &WamReg,
    functor: &String,
    arity: &usize,
    registers: &mut Registers,
    heap: &mut CellHeap,
    layered_uf: &mut LayeredUf,
    read_write_mode: &mut ReadWriteMode,
    exec_mode: &mut ExecMode,
) {
    match heap.value(existing_cell).as_ref() {
        Cell::Var { .. } => {
            let struct_cell = heap.insert_struct(functor, *arity);
            let id = layered_uf.alloc_node();
            registers.set_register(op_reg, id);
            layered_uf.union(existing_parent_index, id);
            layered_uf.set_cell(id, struct_cell);
            *read_write_mode = ReadWriteMode::Write;
        }
        Cell::Struct {
            functor: existing_functor,
            arity: existing_arity,
        } => {
            if existing_functor == functor && existing_arity == arity {
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, existing_cell);
                registers.set_register(op_reg, uf_id);
                let local_root = layered_uf.find_root(existing_parent_index);
                *read_write_mode =
                    ReadWriteMode::Read(GlobalParentIndex::offset(local_root.rooted, 1));
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
                let cell_id = heap.insert_struct(functor, *arity);
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set_register(reg, uf_id);
            }

            WamInstr::SetVar { name, reg } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set_register(reg, uf_id);
            }

            WamInstr::SetVal { reg, .. } => {
                let prev_id = registers.get_register(reg);
                if !prev_id.is_empty() {
                    let new_id = layered_uf.alloc_node();
                    layered_uf.union(prev_id, new_id);
                } else {
                    panic!("SetVal: register is empty");
                }
            }

            WamInstr::PutVar {
                name, argreg: reg, ..
            } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set_register(reg, uf_id);
            }

            WamInstr::GetStruct {
                functor,
                arity,
                reg: op_reg,
            } => {
                let id = registers.get_register(op_reg);
                if !id.is_empty() {
                    let Parent { cell, rooted, .. } = layered_uf.find_root(id);
                    getstruct_cell_ref(
                        *rooted,
                        *cell,
                        op_reg,
                        functor,
                        arity,
                        registers,
                        heap,
                        layered_uf,
                        read_write_mode,
                        exec_mode,
                    );
                } else {
                    panic!("GetStruct: register is empty");
                }
            }

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
                    ReadWriteMode::Read(read_index) => {
                        // read and set to register
                        registers.set_register(reg, *read_index);

                        *read_write_mode =
                            ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                    }
                    ReadWriteMode::Write => {
                        // set
                        let cell_id = heap.insert_var(name.clone());
                        let uf_id = layered_uf.alloc_node();
                        layered_uf.set_cell(uf_id, cell_id);
                        registers.set_register(reg, uf_id);
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
            &mut ReadWriteMode::Write,
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

    #[test]
    fn var_to_var_binding() {
        let (query, query_term) = compile_program("honi(X).", "honi(Y).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "honi".to_string(),
            vec![Term::new_var("X".to_string())],
        )];
        assert_eq!(result, Ok(expected));
    }
}
