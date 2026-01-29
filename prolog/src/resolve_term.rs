use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compiler_bytecode::WamReg;
use crate::interpreter::{Registers, StackFrame, get_reg, resolve_register};
use crate::layered_uf::{GlobalParentIndex, LayeredUf};
use crate::parse::{Term, TermId};
use std::collections::HashMap;

/// Termを走査して変数を解決済みの値で置き換える
pub fn resolve_term(
    term: &Term,
    term_to_reg: &HashMap<TermId, WamReg>,
    registers: &mut Registers,
    heap: &CellHeap,
    uf: &mut LayeredUf,
    call_stack: &Vec<StackFrame>,
) -> Term {
    match term {
        Term::Var { id, name, .. } => {
            if let Some(reg) = term_to_reg.get(id) {
                let uf_id =
                    resolve_register(call_stack, registers, get_reg(registers, call_stack, reg, usize::MAX), usize::MAX);
                let root = uf.find_root(uf_id);
                // rootedを使う: 構造体の引数はroot.rooted + 1から連続している
                cell_to_term(root.cell, root.rooted, heap, uf)
            } else {
                // term_to_regに登録されていない変数はそのまま
                Term::new_var(name.clone())
            }
        }
        Term::Struct { functor, args, .. } => {
            let resolved_args = args
                .iter()
                .map(|arg| resolve_term(arg, term_to_reg, registers, heap, uf, call_stack))
                .collect();
            Term::new_struct(functor.clone(), resolved_args)
        }
        Term::Number { value, .. } => Term::new_number(*value),
        Term::List { items, tail, .. } => {
            let resolved_items = items
                .iter()
                .map(|item| resolve_term(item, term_to_reg, registers, heap, uf, call_stack))
                .collect();
            let resolved_tail = tail.as_ref().map(|t| {
                Box::new(resolve_term(
                    t,
                    term_to_reg,
                    registers,
                    heap,
                    uf,
                    call_stack,
                ))
            });
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
        Cell::VarRef { ref_index, .. } => cell_to_term(*ref_index, uf_id, heap, uf),
    }
}
