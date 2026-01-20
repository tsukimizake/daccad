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
    pub(crate) fn get(&self, reg: &WamReg) -> GlobalParentIndex {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        self.registers
            .get(index)
            .copied()
            .unwrap_or(GlobalParentIndex::EMPTY)
    }

    #[inline]
    pub fn set(&mut self, reg: &WamReg, value: GlobalParentIndex) {
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
            let new_id = layered_uf.alloc_node();
            registers.set(op_reg, new_id);
            layered_uf.union(existing_parent_index, new_id);
            layered_uf.set_cell(new_id, struct_cell);
            heap.set_ref(existing_cell, struct_cell);
            *read_write_mode = ReadWriteMode::Write;
        }
        Cell::Struct {
            functor: existing_functor,
            arity: existing_arity,
        } => {
            if existing_functor == functor && existing_arity == arity {
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, existing_cell);
                registers.set(op_reg, uf_id);
                let local_root = layered_uf.find_root(existing_parent_index);
                *read_write_mode =
                    ReadWriteMode::Read(GlobalParentIndex::offset(local_root.rooted, 1));
            } else {
                *exec_mode = ExecMode::ResolvedToFalse;
            }
        }
        Cell::VarRef { ref_index, .. } => {
            getstruct_cell_ref(
                existing_parent_index,
                *ref_index,
                op_reg,
                functor,
                arity,
                registers,
                heap,
                layered_uf,
                read_write_mode,
                exec_mode,
            );
        }
    }
}

/// 2つのUFノードを統一する
/// - Var + Var: union (r がルートになる)
/// - Var + Struct: Var を Struct への参照に変更し、Struct がルートになるように union
/// - Struct + Struct: functor/arity が一致すれば成功（引数の再帰的統一は未実装）、不一致なら失敗
///
/// 戻り値: 統一成功なら true、失敗なら false

/// CellIndex から VarRef を辿って実際のセルを取得
fn deref_cell(cell: CellIndex, heap: &CellHeap) -> CellIndex {
    match heap.value(cell).as_ref() {
        Cell::VarRef { ref_index, .. } => deref_cell(*ref_index, heap),
        _ => cell,
    }
}

fn unify(
    l_id: GlobalParentIndex,
    r_id: GlobalParentIndex,
    heap: &CellHeap,
    uf: &mut LayeredUf,
) -> bool {
    let mut ids_to_unify = Vec::with_capacity(10);
    ids_to_unify.push((l_id, r_id));
    let mut success = true;

    while (!ids_to_unify.is_empty()) && success {
        let (l_id, r_id) = ids_to_unify.pop().unwrap();
        let l_cell = deref_cell(uf.find_root(l_id).cell, heap);
        let r_cell = deref_cell(uf.find_root(r_id).cell, heap);
        match (heap.value(l_cell).as_ref(), heap.value(r_cell).as_ref()) {
            // Var + Var: そのまま union (l <- r で r がルート)
            (Cell::Var { .. }, Cell::Var { .. }) => {
                uf.union(l_id, r_id);
                success = true
            }
            // Var + Struct: Struct がルートになるように union
            (Cell::Var { .. }, Cell::Struct { .. }) => {
                uf.union(l_id, r_id); // l <- r で r (Struct) がルート
                success = true
            }
            // Struct + Var: Struct がルートになるように union
            (Cell::Struct { .. }, Cell::Var { .. }) => {
                uf.union(r_id, l_id); // r <- l で l (Struct) がルート
                success = true
            }
            // Struct + Struct: functor/arity チェック
            (
                Cell::Struct {
                    functor: f1,
                    arity: a1,
                },
                Cell::Struct {
                    functor: f2,
                    arity: a2,
                },
            ) => {
                if f1 == f2 && a1 == a2 {
                    uf.union(l_id, r_id);

                    // 引数の再帰的unify
                    for i in 0..*a1 {
                        ids_to_unify.push((
                            GlobalParentIndex::offset(l_id, 1 + i),
                            GlobalParentIndex::offset(r_id, 1 + i),
                        ));
                    }
                    success = true
                } else {
                    success = false
                }
            }
            // VarRef は deref_cell で解決されているはずなので到達しない
            _ => {
                panic!(
                    "unify: unexpected cell combination: {:?} and {:?}",
                    heap.value(l_cell),
                    heap.value(r_cell)
                );
            }
        }
    }
    success
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
                registers.set(reg, uf_id);
            }

            WamInstr::SetVar { name, reg } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                registers.set(reg, uf_id);
            }

            WamInstr::SetVal { reg, .. } => {
                let prev_id = registers.get(reg);
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
                registers.set(reg, uf_id);
            }

            WamInstr::GetStruct {
                functor,
                arity,
                reg: op_reg,
            } => {
                let id = registers.get(op_reg);
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
                        // just reference from register
                        registers.set(reg, *read_index);

                        *read_write_mode =
                            ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                    }
                    ReadWriteMode::Write => {
                        // set
                        let cell_id = heap.insert_var(name.clone());
                        let uf_id = layered_uf.alloc_node();
                        layered_uf.set_cell(uf_id, cell_id);
                        registers.set(reg, uf_id);
                    }
                }
            }
            WamInstr::UnifyVal { reg, .. } => match read_write_mode {
                ReadWriteMode::Read(read_index) => {
                    let id = registers.get(reg);
                    layered_uf.union(id, *read_index);
                    *read_write_mode =
                        ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                }
                ReadWriteMode::Write => {
                    let id = registers.get(reg);
                    let new_id = layered_uf.alloc_node();
                    layered_uf.union(id, new_id);
                }
            },

            WamInstr::Label { name: _, arity: _ } => {}
            WamInstr::Proceed => {
                // stack.pop();
                // if stack.len() == 0 {
                *exec_mode = ExecMode::ResolvedToTrue;
                // }
            }
            WamInstr::Error { message } => {
                *exec_mode = ExecMode::ResolvedToFalse;
                println!("Error instruction executed: {}", message);
            }

            // トップレベル引数の変数の初回出現。レジスタには既にクエリからの値が入っている。
            WamInstr::GetVar { name, reg, with } => {
                let reg_id = registers.get(reg);
                debug_assert!(!reg_id.is_empty(), "GetVar: register is empty");

                // DB側の変数用に新しいノードとセルを作成
                let db_var_cell = heap.insert_var(name.clone());
                let with_id = layered_uf.alloc_node();
                layered_uf.set_cell(with_id, db_var_cell);
                registers.set(with, with_id);

                if !unify(reg_id, with_id, heap, layered_uf) {
                    println!("  unify failed!");
                    *exec_mode = ExecMode::ResolvedToFalse;
                }
            }

            // トップレベル引数の変数の2回目以降の出現
            WamInstr::GetVal { with, reg, .. } => {
                let with_id = registers.get(with);
                let reg_id = registers.get(reg);
                if !unify(with_id, reg_id, heap, layered_uf) {
                    *exec_mode = ExecMode::ResolvedToFalse;
                }
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

    #[test]
    fn multiple_usages_of_same_variable() {
        let (query, query_term) = compile_program("likes(X, X).", "likes(fuwa, Y).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "likes".to_string(),
            vec![
                Term::new_struct("fuwa".to_string(), vec![]),
                Term::new_struct("fuwa".to_string(), vec![]),
            ],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn deep_struct_on_db() {
        let (query, query_term) = compile_program("a(b(c)).", "a(X).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "a".to_string(),
            vec![Term::new_struct(
                "b".to_string(),
                vec![Term::new_struct("c".to_string(), vec![])],
            )],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn deep_struct_on_query() {
        let (query, query_term) = compile_program("a(X).", "a(b(c)).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "a".to_string(),
            vec![Term::new_struct(
                "b".to_string(),
                vec![Term::new_struct("c".to_string(), vec![])],
            )],
        )];
        assert_eq!(result, Ok(expected));
    }

    // 再帰的unifyのテスト

    #[test]
    fn recursive_unify_nested_struct_match() {
        // 両方同じネスト構造 -> 成功
        let (query, query_term) = compile_program("f(a(b)).", "f(a(b)).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_inner() {
        // 内側の引数が異なる -> 失敗
        let (query, query_term) = compile_program("f(a(b)).", "f(a(c)).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_functor() {
        // 内側のファンクタが異なる -> 失敗
        let (query, query_term) = compile_program("f(a(b)).", "f(c(b)).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn recursive_unify_var_in_nested_struct() {
        // DBの変数がネスト構造内の値に束縛される
        let (query, query_term) = compile_program("f(a(X)).", "f(a(b)).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn recursive_unify_query_var_binds_in_nested() {
        // クエリ変数がネスト構造内の値に束縛される
        let (query, query_term) = compile_program("f(a(b)).", "f(a(X)).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "f".to_string(),
            vec![Term::new_struct(
                "a".to_string(),
                vec![Term::new_struct("b".to_string(), vec![])],
            )],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn recursive_unify_multiple_args() {
        // 複数引数でそれぞれネスト構造を持つ
        let (query, query_term) = compile_program("f(a(b), c(d)).", "f(a(b), c(d)).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn recursive_unify_multiple_args_one_mismatch() {
        // 複数引数の一つがミスマッチ
        let (query, query_term) = compile_program("f(a(b), c(d)).", "f(a(b), c(e)).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn recursive_unify_three_levels_deep() {
        // 3段階のネスト
        let (query, query_term) = compile_program("f(a(b(c))).", "f(a(b(c))).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn recursive_unify_three_levels_deep_mismatch() {
        // 3段階のネストで最深部がミスマッチ
        let (query, query_term) = compile_program("f(a(b(c))).", "f(a(b(d))).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn recursive_unify_var_at_deep_level() {
        // 深い位置の変数が束縛される
        let (query, query_term) = compile_program("f(a(b(X))).", "f(a(b(c))).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }
}
