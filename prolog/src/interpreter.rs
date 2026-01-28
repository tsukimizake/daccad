use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compile_query::CompiledQuery;
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::{GlobalParentIndex, LayeredUf, Parent};
use crate::parse::Term;
use crate::resolve_term::resolve_term;

type XReg = GlobalParentIndex;
type YReg = GlobalParentIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registers {
    regs: Vec<XReg>,
}

impl Registers {
    fn new() -> Self {
        Self {
            regs: vec![GlobalParentIndex::EMPTY; 32],
        }
    }
}

#[inline]
pub(crate) fn get_reg(
    registers: &Registers,
    call_stack: &[StackFrame],
    reg: &WamReg,
) -> GlobalParentIndex {
    match reg {
        WamReg::X(index) => registers
            .regs
            .get(*index)
            .copied()
            .unwrap_or(GlobalParentIndex::EMPTY),
        WamReg::Y(index) => call_stack
            .last()
            .and_then(|frame| frame.regs.get(*index).copied())
            .unwrap_or(GlobalParentIndex::EMPTY),
    }
}

#[inline]
pub(crate) fn set_reg(
    registers: &mut Registers,
    call_stack: &mut [StackFrame],
    reg: &WamReg,
    value: GlobalParentIndex,
) {
    match reg {
        WamReg::X(index) => {
            if *index >= registers.regs.len() {
                registers.regs.resize(*index + 1, GlobalParentIndex::EMPTY);
            }
            registers.regs[*index] = value;
        }
        WamReg::Y(index) => {
            if let Some(frame) = call_stack.last_mut() {
                if *index >= frame.regs.len() {
                    frame.regs.resize(*index + 1, GlobalParentIndex::EMPTY);
                }
                frame.regs[*index] = value;
            } else {
                panic!("set_reg Y: call_stack is empty");
            }
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
enum ExecMode {
    Continue,
    ResolvedToTrue,
    ResolvedToFalse,
}

#[derive(Debug, Clone)]
pub(crate) struct StackFrame {
    return_address: usize,
    pub(crate) regs: Vec<YReg>,
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
    call_stack: &mut [StackFrame],
) {
    match heap.value(existing_cell).as_ref() {
        Cell::Var { .. } => {
            let struct_cell = heap.insert_struct(functor, *arity);
            let new_id = layered_uf.alloc_node();
            set_reg(registers, call_stack, op_reg, new_id);
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
                set_reg(registers, call_stack, op_reg, uf_id);
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
                call_stack,
            );
        }
    }
}

/// CellIndex から VarRef を辿って実際のセルを取得
fn deref_cell(cell: CellIndex, heap: &CellHeap) -> CellIndex {
    match heap.value(cell).as_ref() {
        Cell::VarRef { ref_index, .. } => deref_cell(*ref_index, heap),
        _ => cell,
    }
}

/// 2つのUFノードを統一する
/// - Var + Var: union (r がルートになる)
/// - Var + Struct: Var を Struct への参照に変更し、Struct がルートになるように union
/// - Struct + Struct: functor/arity が一致すれば成功（引数の再帰的統一は未実装）、不一致なら失敗
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
    call_stack: &mut Vec<StackFrame>,
) {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutStruct {
                functor,
                arity,
                arg_reg,
            } => {
                let cell_id = heap.insert_struct(functor, *arity);
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                set_reg(registers, call_stack, arg_reg, uf_id);
            }

            WamInstr::SetVar { name, reg } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                set_reg(registers, call_stack, reg, uf_id);
            }

            WamInstr::SetVal { reg, .. } => {
                let prev_id = get_reg(registers, call_stack, reg);
                if !prev_id.is_empty() {
                    let new_id = layered_uf.alloc_node();
                    layered_uf.union(new_id, prev_id);
                } else {
                    panic!("SetVal: register is empty");
                }
            }

            WamInstr::PutVar {
                name,
                arg_reg,
                with,
                ..
            } => {
                let cell_id = heap.insert_var(name.clone());
                let uf_id = layered_uf.alloc_node();
                layered_uf.set_cell(uf_id, cell_id);
                set_reg(registers, call_stack, arg_reg, uf_id);
                set_reg(registers, call_stack, with, uf_id);
            }

            WamInstr::PutVal { arg_reg, with, .. } => {
                // with レジスタの内容を arg_reg にコピー
                let with_id = get_reg(registers, call_stack, with);
                if !with_id.is_empty() {
                    set_reg(registers, call_stack, arg_reg, with_id);
                } else {
                    panic!("PutVal: with register is empty");
                }
            }

            WamInstr::GetStruct {
                functor,
                arity,
                reg: op_reg,
            } => {
                let id = get_reg(registers, call_stack, op_reg);
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
                        call_stack,
                    );
                } else {
                    panic!("GetStruct: register is empty");
                }
            }

            WamInstr::UnifyVar { name, reg } => {
                match read_write_mode {
                    ReadWriteMode::Read(read_index) => {
                        // just reference from register
                        set_reg(registers, call_stack, reg, *read_index);

                        *read_write_mode =
                            ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                    }
                    ReadWriteMode::Write => {
                        // set
                        let cell_id = heap.insert_var(name.clone());
                        let uf_id = layered_uf.alloc_node();
                        layered_uf.set_cell(uf_id, cell_id);
                        set_reg(registers, call_stack, reg, uf_id);
                    }
                }
            }
            WamInstr::UnifyVal { reg, .. } => match read_write_mode {
                ReadWriteMode::Read(read_index) => {
                    let id = get_reg(registers, call_stack, reg);
                    layered_uf.union(id, *read_index);
                    *read_write_mode =
                        ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                }
                ReadWriteMode::Write => {
                    let id = get_reg(registers, call_stack, reg);
                    let new_id = layered_uf.alloc_node();
                    layered_uf.union(id, new_id);
                }
            },

            WamInstr::Call {
                predicate: _,
                arity: _,
                to_program_counter,
            } => {
                println!("push_stack_frame called stack: {:?}", call_stack);
                // 現在のフレームに return_address を設定
                if let Some(frame) = call_stack.last_mut() {
                    frame.return_address = *program_counter;
                }
                *program_counter = *to_program_counter;
            }
            WamInstr::Label { name: _, arity: _ } => {}
            WamInstr::Proceed => {
                println!("pop_stack_frame called stack: {:?}", call_stack);
                // クエリのフレームだけが残っている場合は終了
                if call_stack.len() <= 1 {
                    *exec_mode = ExecMode::ResolvedToTrue;
                } else if let Some(frame) = call_stack.last() {
                    *program_counter = frame.return_address;
                } else {
                    panic!("Proceed on empty call_stack")
                }
            }
            WamInstr::Error { message: _ } => {
                *exec_mode = ExecMode::ResolvedToFalse;
            }

            // トップレベル引数の変数の初回出現。レジスタには既にクエリからの値が入っている。
            WamInstr::GetVar { name, reg, with } => {
                let reg_id = get_reg(registers, call_stack, reg);
                debug_assert!(!reg_id.is_empty(), "GetVar: register is empty");

                // DB側の変数用に新しいノードとセルを作成
                let db_var_cell = heap.insert_var(name.clone());
                let with_id = layered_uf.alloc_node();
                layered_uf.set_cell(with_id, db_var_cell);
                set_reg(registers, call_stack, with, with_id);

                if !unify(reg_id, with_id, heap, layered_uf) {
                    *exec_mode = ExecMode::ResolvedToFalse;
                }
            }

            // トップレベル引数の変数の2回目以降の出現
            WamInstr::GetVal { with, reg, .. } => {
                let with_id = get_reg(registers, call_stack, with);
                let reg_id = get_reg(registers, call_stack, reg);
                if !unify(with_id, reg_id, heap, layered_uf) {
                    *exec_mode = ExecMode::ResolvedToFalse;
                }
            }

            // スタックフレームを確保してYレジスタを初期化
            WamInstr::Allocate { size } => {
                call_stack.push(StackFrame {
                    return_address: 0,
                    regs: Vec::with_capacity(*size),
                });
            }
            WamInstr::Deallocate => {
                // フレームをポップ
                println!("deallocate called stack: {:?}", call_stack);
                call_stack.pop();
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
    let mut read_write_mode = ReadWriteMode::Write;
    let mut call_stack: Vec<StackFrame> = Vec::with_capacity(100);

    call_stack.push(StackFrame {
        return_address: 0,
        regs: Vec::with_capacity(32),
    });

    while exec_mode == ExecMode::Continue {
        exectute_impl(
            &query.instructions,
            &mut program_counter,
            &mut registers,
            &mut cell_heap,
            &mut layered_uf,
            &mut exec_mode,
            &mut read_write_mode,
            &mut call_stack,
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
                &call_stack,
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

    #[test]
    fn sample_rule() {
        let (query, query_term) =
            compile_program("p(X,Y) :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p(A, B).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "p".to_string(),
            vec![
                Term::new_struct("a".to_string(), vec![]),
                Term::new_struct("c".to_string(), vec![]),
            ],
        )];
        assert_eq!(result, Ok(expected));
    }

    // ===== ルールのテストケース =====

    #[test]
    fn rule_single_goal() {
        let (query, query_term) =
            compile_program("parent(X) :- father(X). father(tom).", "parent(tom).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_single_goal_with_var_query() {
        let (query, query_term) =
            compile_program("parent(X) :- father(X). father(tom).", "parent(Y).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "parent".to_string(),
            vec![Term::new_struct("tom".to_string(), vec![])],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn rule_multiple_goals() {
        let (query, query_term) = compile_program(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, c).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_multiple_goals_with_var() {
        let (query, query_term) = compile_program(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, W).",
        );
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "grandparent".to_string(),
            vec![
                Term::new_struct("a".to_string(), vec![]),
                Term::new_struct("c".to_string(), vec![]),
            ],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn rule_fails_first_subgoal() {
        let (query, query_term) = compile_program("p(X) :- q(X), r(X). q(b). r(a).", "p(a).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn rule_fails_second_subgoal() {
        let (query, query_term) = compile_program("p(X) :- q(X), r(X). q(a). r(b).", "p(a).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn rule_fails_no_matching_fact() {
        let (query, query_term) = compile_program("p(X) :- q(X). q(a).", "p(b).");
        let result = execute_instructions(query, query_term);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn rule_chain_two_levels() {
        let (query, query_term) = compile_program("a(X) :- b(X). b(X) :- c(X). c(foo).", "a(foo).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_chain_three_levels() {
        let (query, query_term) = compile_program(
            "a(X) :- b(X). b(X) :- c(X). c(X) :- d(X). d(bar).",
            "a(bar).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_chain_with_var_binding() {
        let (query, query_term) = compile_program("a(X) :- b(X). b(X) :- c(X). c(baz).", "a(Y).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "a".to_string(),
            vec![Term::new_struct("baz".to_string(), vec![])],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn rule_with_nested_struct_in_fact() {
        let (query, query_term) = compile_program(
            "outer(X) :- inner(X). inner(pair(a, b)).",
            "outer(pair(a, b)).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_with_nested_struct_var_binding() {
        let (query, query_term) =
            compile_program("outer(X) :- inner(X). inner(pair(a, b)).", "outer(Y).");
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "outer".to_string(),
            vec![Term::new_struct(
                "pair".to_string(),
                vec![
                    Term::new_struct("a".to_string(), vec![]),
                    Term::new_struct("b".to_string(), vec![]),
                ],
            )],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn rule_with_deeply_nested_struct() {
        let (query, query_term) = compile_program(
            "wrap(X) :- data(X). data(node(leaf(a), leaf(b))).",
            "wrap(node(leaf(a), leaf(b))).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_shared_variable_in_body() {
        let (query, query_term) = compile_program("same(X) :- eq(X, X). eq(a, a).", "same(a).");
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_shared_variable_propagation() {
        let (query, query_term) = compile_program(
            "connect(X, Z) :- link(X, Y), link(Y, Z). link(a, b). link(b, c).",
            "connect(a, Z).",
        );
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "connect".to_string(),
            vec![
                Term::new_struct("a".to_string(), vec![]),
                Term::new_struct("c".to_string(), vec![]),
            ],
        )];
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn rule_three_args() {
        let (query, query_term) = compile_program(
            "triple(X, Y, Z) :- first(X), second(Y), third(Z). first(a). second(b). third(c).",
            "triple(A, B, C).",
        );
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "triple".to_string(),
            vec![
                Term::new_struct("a".to_string(), vec![]),
                Term::new_struct("b".to_string(), vec![]),
                Term::new_struct("c".to_string(), vec![]),
            ],
        )];
        assert_eq!(result, Ok(expected));
    }

    // バックトラックが必要（同じ述語に複数のファクトがある）
    #[test]
    #[ignore]
    fn rule_mixed_with_facts() {
        let (query, query_term) = compile_program(
            "animal(dog). animal(cat). is_pet(X) :- animal(X).",
            "is_pet(dog).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_head_with_struct() {
        let (query, query_term) = compile_program(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(pair(a, b)).",
        );
        let result = execute_instructions(query, query_term.clone());
        assert_eq!(result, Ok(query_term));
    }

    #[test]
    fn rule_head_with_struct_var_query() {
        let (query, query_term) = compile_program(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(P).",
        );
        let result = execute_instructions(query, query_term);
        let expected = vec![Term::new_struct(
            "make_pair".to_string(),
            vec![Term::new_struct(
                "pair".to_string(),
                vec![
                    Term::new_struct("a".to_string(), vec![]),
                    Term::new_struct("b".to_string(), vec![]),
                ],
            )],
        )];
        assert_eq!(result, Ok(expected));
    }
}
