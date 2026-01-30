use crate::cell_heap::{Cell, CellHeap, CellIndex};
use crate::compile_query::CompiledQuery;
use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::layered_uf::{GlobalParentIndex, LayeredUf, Parent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Register {
    UfRef(GlobalParentIndex),
    XRef(usize),
    YRef(usize),
}

impl From<WamReg> for Register {
    fn from(reg: WamReg) -> Self {
        match reg {
            WamReg::X(index) => Register::XRef(index),
            WamReg::Y(index) => Register::YRef(index),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    regs: Vec<Register>,
}

impl Registers {
    fn new() -> Self {
        Self {
            regs: Vec::with_capacity(32),
        }
    }
}

/// レジスタの値を取得する。XRefの場合は再帰的に辿り、最終的にUfRefを返す。
/// UfRefでない場合はpanic!する。
#[inline]
pub(crate) fn get_reg<'a>(
    registers: &'a Registers,
    call_stack: &'a [StackFrame],
    reg: &WamReg,
    program_counter: usize,
) -> &'a Register {
    match reg {
        WamReg::X(index) => registers
            .regs
            .get(*index)
            .unwrap_or_else(|| panic!("get_reg: X register is empty at pc={}", program_counter)),
        WamReg::Y(index) => call_stack
            .last()
            .and_then(|frame| frame.regs.get(*index))
            .unwrap_or_else(|| panic!("get_reg: Y register is empty at pc={}", program_counter)),
    }
}

/// Registerを再帰的に辿り、UfRefを返す。
/// YRefの場合は参照先を辿る。
pub(crate) fn resolve_register(
    call_stack: &Vec<StackFrame>,
    registers: &Registers,
    register: &Register,
    program_counter: usize,
) -> GlobalParentIndex {
    match register {
        Register::UfRef(id) => *id,
        Register::XRef(index) => {
            let next = registers.regs.get(*index).unwrap_or_else(|| {
                panic!(
                    "resolve_register: XRef points to empty register at pc={}",
                    program_counter
                )
            });
            resolve_register(call_stack, registers, next, program_counter)
        }
        Register::YRef(index) => {
            let next = call_stack
                .last()
                .unwrap()
                .regs
                .get(*index)
                .unwrap_or_else(|| {
                    panic!(
                        "resolve_register: YRef points to empty register at pc={}",
                        program_counter
                    )
                });
            resolve_register(call_stack, registers, next, program_counter)
        }
    }
}

#[inline]
pub(crate) fn set_reg(
    registers: &mut Registers,
    call_stack: &mut [StackFrame],
    reg: &WamReg,
    value: Register,
    program_counter: usize,
) {
    match reg {
        WamReg::X(index) => {
            if *index >= registers.regs.len() {
                registers
                    .regs
                    .resize(*index + 1, Register::UfRef(GlobalParentIndex::EMPTY));
            }
            registers.regs[*index] = value;
        }
        WamReg::Y(index) => {
            if let Some(frame) = call_stack.last_mut() {
                if *index >= frame.regs.len() {
                    frame
                        .regs
                        .resize(*index + 1, Register::UfRef(GlobalParentIndex::EMPTY));
                }
                frame.regs[*index] = value;
            } else {
                panic!("set_reg Y: call_stack is empty at pc={}", program_counter);
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
pub struct StackFrame {
    return_address: usize,
    pub(crate) regs: Vec<Register>,
}

/// 実行後の状態
#[derive(Debug)]
pub struct ExecutionState {
    pub registers: Registers,
    pub cell_heap: CellHeap,
    pub layered_uf: LayeredUf,
    pub call_stack: Vec<StackFrame>,
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
    program_counter: &usize,
) {
    match heap.value(existing_cell).as_ref() {
        Cell::Var { .. } => {
            let struct_cell = heap.insert_struct(functor, *arity);
            let new_id = layered_uf.alloc_node();
            set_reg(
                registers,
                call_stack,
                op_reg,
                Register::UfRef(new_id),
                *program_counter,
            );
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
                set_reg(
                    registers,
                    call_stack,
                    op_reg,
                    Register::UfRef(uf_id),
                    *program_counter,
                );
                let local_root = layered_uf.find_root(existing_parent_index);
                *read_write_mode =
                    ReadWriteMode::Read(GlobalParentIndex::offset(local_root.rooted, 1));
            } else {
                println!(
                    "GetStruct failed: expected ({} / {}), found ({} / {}) at {:?}",
                    functor, arity, existing_functor, existing_arity, program_counter
                );
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
                program_counter,
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
                set_reg(
                    registers,
                    call_stack,
                    arg_reg,
                    Register::UfRef(uf_id),
                    *program_counter,
                );
            }

            WamInstr::SetVar { reg, .. } => match reg {
                WamReg::X(index) => {
                    set_reg(
                        registers,
                        call_stack,
                        reg,
                        Register::XRef(*index),
                        *program_counter,
                    );
                }
                WamReg::Y(_) => {
                    panic!("SetVar arg should be X register at pc={}", program_counter);
                }
            },

            WamInstr::SetVal { reg, .. } => {
                let prev_id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, reg, *program_counter),
                    *program_counter,
                );
                let new_id = layered_uf.alloc_node();
                layered_uf.union(new_id, prev_id);
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

                set_reg(
                    registers,
                    call_stack,
                    arg_reg,
                    Register::UfRef(uf_id),
                    *program_counter,
                );
                set_reg(
                    registers,
                    call_stack,
                    with,
                    Register::UfRef(uf_id),
                    *program_counter,
                );
            }

            WamInstr::PutVal { arg_reg, with, .. } => {
                // with レジスタの内容を arg_reg にコピー
                let with_id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, with, *program_counter),
                    *program_counter,
                );
                set_reg(
                    registers,
                    call_stack,
                    arg_reg,
                    Register::UfRef(with_id),
                    *program_counter,
                );
            }

            // トップレベル引数の変数の初回出現。レジスタには既にクエリからの値が入っている。
            WamInstr::GetVar { reg, with, .. } => {
                let reg_id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, reg, *program_counter),
                    *program_counter,
                );
                set_reg(
                    registers,
                    call_stack,
                    with,
                    Register::UfRef(reg_id),
                    *program_counter,
                );
            }

            // トップレベル引数の変数の2回目以降の出現
            WamInstr::GetVal { with, reg, .. } => {
                let with_id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, with, *program_counter),
                    *program_counter,
                );
                let reg_id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, reg, *program_counter),
                    *program_counter,
                );
                if !unify(with_id, reg_id, heap, layered_uf) {
                    println!(
                        "GetVal unify failed for with_id: {:?}, reg_id: {:?}",
                        with_id, reg_id
                    );
                    *exec_mode = ExecMode::ResolvedToFalse;
                }
            }
            WamInstr::GetStruct {
                functor,
                arity,
                reg: op_reg,
            } => {
                let id = resolve_register(
                    call_stack,
                    registers,
                    get_reg(registers, call_stack, op_reg, *program_counter),
                    *program_counter,
                );
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
                    program_counter,
                );
            }

            WamInstr::UnifyVar { name, reg } => {
                match read_write_mode {
                    ReadWriteMode::Read(read_index) => {
                        // just reference from register
                        set_reg(
                            registers,
                            call_stack,
                            reg,
                            Register::UfRef(*read_index),
                            *program_counter,
                        );

                        *read_write_mode =
                            ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                    }
                    ReadWriteMode::Write => {
                        // set
                        let cell_id = heap.insert_var(name.clone());
                        let uf_id = layered_uf.alloc_node();
                        layered_uf.set_cell(uf_id, cell_id);
                        set_reg(
                            registers,
                            call_stack,
                            reg,
                            Register::UfRef(uf_id),
                            *program_counter,
                        );
                    }
                }
            }
            WamInstr::UnifyVal { reg, .. } => match read_write_mode {
                ReadWriteMode::Read(read_index) => {
                    let id = resolve_register(
                        call_stack,
                        registers,
                        get_reg(registers, call_stack, reg, *program_counter),
                        *program_counter,
                    );
                    layered_uf.union(id, *read_index);
                    *read_write_mode =
                        ReadWriteMode::Read(GlobalParentIndex::offset(*read_index, 1));
                }
                ReadWriteMode::Write => {
                    let id = resolve_register(
                        call_stack,
                        registers,
                        get_reg(registers, call_stack, reg, *program_counter),
                        *program_counter,
                    );
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
                    println!("call set return_address: {}", frame.return_address);
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
                    println!("proceed to return_address: {}", frame.return_address);
                    *program_counter = frame.return_address;
                } else {
                    panic!("Proceed on empty call_stack at pc={}", program_counter)
                }
            }
            WamInstr::Error { message: _ } => {
                println!("Error instruction encountered");
                *exec_mode = ExecMode::ResolvedToFalse;
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
                todo!("{:?} at pc={}", instr, program_counter);
            }
        }
    } else {
        todo!("current_instr is None at pc={}", program_counter);
    }
}

pub fn execute_instructions(query: &CompiledQuery) -> Result<ExecutionState, ()> {
    let mut program_counter = 0;
    let mut exec_mode = ExecMode::Continue;
    let mut layered_uf = LayeredUf::new();
    let mut cell_heap = CellHeap::new();
    let mut registers = Registers::new();
    let mut read_write_mode = ReadWriteMode::Write;
    let mut call_stack: Vec<StackFrame> = Vec::with_capacity(100);

    // クエリの最初の Allocate 命令でスタックフレームが作成されるので、
    // ここでは初期フレームを作成しない

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
        println!("pc: {}", program_counter);
        program_counter += 1;
    }

    if exec_mode == ExecMode::ResolvedToFalse {
        println!("registers after execution: {:?}", registers);
        println!("call_stack after execution: {:?}", call_stack);
        println!("layered_uf after execution: {:?}", layered_uf);
        println!("cell_heap after execution: {:?}", cell_heap);
        return Err(());
    }

    println!("registers after execution: {:?}", registers);
    println!("call_stack after execution: {:?}", call_stack);
    println!("layered_uf after execution: {:?}", layered_uf);
    println!("cell_heap after execution: {:?}", cell_heap);

    Ok(ExecutionState {
        registers,
        cell_heap,
        layered_uf,
        call_stack,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cell_heap::Cell, compile_db::compile_db, compile_link::compile_link,
        compile_query::compile_query, compiler_bytecode::WamInstrs, parse,
    };

    fn compile_program(db_src: &str, query_src: &str) -> CompiledQuery {
        let db_clauses = parse::database(db_src).expect("failed to parse db");
        let query_terms = parse::query(query_src).expect("failed to parse query").1;
        let linked = compile_link(
            compile_query(query_terms.clone()),
            compile_db(db_clauses.clone()),
        );
        println!("{:?}", WamInstrs(&linked.instructions));
        linked
    }

    /// Y レジスタから Cell を取得するヘルパー
    fn get_y_cell(state: &mut ExecutionState, y_index: usize) -> Cell {
        let register = state
            .call_stack
            .last()
            .and_then(|frame| frame.regs.get(y_index))
            .expect("Y register not found");
        let uf_id = resolve_register(&state.call_stack, &state.registers, register, usize::MAX);
        let root = state.layered_uf.find_root(uf_id);
        (*state.cell_heap.value(root.cell)).clone()
    }

    #[test]
    fn simple_atom_match() {
        // DB: hello. Query: hello.
        // クエリに変数なし、成功のみ確認
        let query = compile_program("hello.", "hello.");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn fail_unmatched() {
        // DB: hello. Query: bye.
        // 失敗することを確認
        let query = compile_program("hello.", "bye.");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn db_var_matches_constant_query() {
        // DB: honi(X). Query: honi(fuwa).
        // クエリに変数なし、成功のみ確認
        let query = compile_program("honi(X).", "honi(fuwa).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn query_var_binds_to_constant_fact() {
        // DB: honi(fuwa). Query: honi(X).
        // X = Y(0) が "fuwa" に束縛される
        let query = compile_program("honi(fuwa).", "honi(X).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "fuwa".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn var_to_var_binding() {
        // DB: honi(X). Query: honi(Y).
        // Y = Y(0) が変数 X に束縛される
        let query = compile_program("honi(X).", "honi(Y).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Var {
                name: "X".to_string()
            }
        );
    }

    #[test]
    fn multiple_usages_of_same_variable() {
        // DB: likes(X, X). Query: likes(fuwa, Y).
        // Y = Y(0) が "fuwa" に束縛される
        let query = compile_program("likes(X, X).", "likes(fuwa, Y).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "fuwa".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn deep_struct_on_db() {
        // DB: a(b(c)). Query: a(X).
        // X = Y(0) が b(c) に束縛される
        let query = compile_program("a(b(c)).", "a(X).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "b".to_string(),
                arity: 1
            }
        );
    }

    #[test]
    fn deep_struct_on_query() {
        // DB: a(X). Query: a(b(c)).
        // クエリに変数なし、成功のみ確認
        let query = compile_program("a(X).", "a(b(c)).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    // 再帰的unifyのテスト

    #[test]
    fn recursive_unify_nested_struct_match() {
        // 両方同じネスト構造 -> 成功
        let query = compile_program("f(a(b)).", "f(a(b)).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_inner() {
        // 内側の引数が異なる -> 失敗
        let query = compile_program("f(a(b)).", "f(a(c)).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn recursive_unify_nested_struct_mismatch_functor() {
        // 内側のファンクタが異なる -> 失敗
        let query = compile_program("f(a(b)).", "f(c(b)).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn recursive_unify_var_in_nested_struct() {
        // DBの変数がネスト構造内の値に束縛される
        let query = compile_program("f(a(X)).", "f(a(b)).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn recursive_unify_query_var_binds_in_nested() {
        // DB: f(a(b)). Query: f(a(X)).
        // X = Y(0) が "b" に束縛される
        let query = compile_program("f(a(b)).", "f(a(X)).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "b".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn recursive_unify_multiple_args() {
        // 複数引数でそれぞれネスト構造を持つ
        let query = compile_program("f(a(b), c(d)).", "f(a(b), c(d)).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn recursive_unify_multiple_args_one_mismatch() {
        // 複数引数の一つがミスマッチ
        let query = compile_program("f(a(b), c(d)).", "f(a(b), c(e)).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn recursive_unify_three_levels_deep() {
        // 3段階のネスト
        let query = compile_program("f(a(b(c))).", "f(a(b(c))).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn recursive_unify_three_levels_deep_mismatch() {
        // 3段階のネストで最深部がミスマッチ
        let query = compile_program("f(a(b(c))).", "f(a(b(d))).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn recursive_unify_var_at_deep_level() {
        // 深い位置の変数が束縛される
        let query = compile_program("f(a(b(X))).", "f(a(b(c))).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn simpler_rule() {
        // Query: p. (変数なし)
        let query = compile_program("p :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p.");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn sample_rule() {
        // Query: p(A, B). A=Y(0), B=Y(1)
        let query = compile_program("p(X,Y) :- q(X, Z), r(Z, Y). q(a, b). r(b, c).", "p(A, B).");
        let mut state = execute_instructions(&query).unwrap();
        println!("state: {:?}", state);
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "a".to_string(),
                arity: 0
            }
        );
        assert_eq!(
            get_y_cell(&mut state, 1),
            Cell::Struct {
                functor: "c".to_string(),
                arity: 0
            }
        );
    }

    // ===== ルールのテストケース =====

    #[test]
    fn rule_single_goal() {
        // Query: parent(tom). (変数なし)
        let query = compile_program("parent(X) :- father(X). father(tom).", "parent(tom).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_single_goal_with_var_query() {
        // Query: parent(Y). Y=Y(0) が "tom" に束縛
        let query = compile_program("parent(X) :- father(X). father(tom).", "parent(Y).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "tom".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn rule_multiple_goals() {
        // Query: grandparent(a, c). (変数なし)
        let query = compile_program(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, c).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_multiple_goals_with_var() {
        // Query: grandparent(a, W). W=Y(0) が "c" に束縛
        let query = compile_program(
            "grandparent(X, Z) :- parent(X, Y), parent(Y, Z). parent(a, b). parent(b, c).",
            "grandparent(a, W).",
        );
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "c".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn rule_fails_first_subgoal() {
        let query = compile_program("p(X) :- q(X), r(X). q(b). r(a).", "p(a).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn rule_fails_second_subgoal() {
        let query = compile_program("p(X) :- q(X), r(X). q(a). r(b).", "p(a).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn rule_fails_no_matching_fact() {
        let query = compile_program("p(X) :- q(X). q(a).", "p(b).");
        let result = execute_instructions(&query);
        assert!(result.is_err());
    }

    #[test]
    fn rule_chain_two_levels() {
        let query = compile_program("a(X) :- b(X). b(X) :- c(X). c(foo).", "a(foo).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_chain_three_levels() {
        let query = compile_program(
            "a(X) :- b(X). b(X) :- c(X). c(X) :- d(X). d(bar).",
            "a(bar).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_chain_with_var_binding() {
        // Query: a(Y). Y=Y(0) が "baz" に束縛
        let query = compile_program("a(X) :- b(X). b(X) :- c(X). c(baz).", "a(Y).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "baz".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn rule_with_nested_struct_in_fact() {
        let query = compile_program(
            "outer(X) :- inner(X). inner(pair(a, b)).",
            "outer(pair(a, b)).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_with_nested_struct_var_binding() {
        // Query: outer(Y). Y=Y(0) が pair(a, b) に束縛
        let query = compile_program("outer(X) :- inner(X). inner(pair(a, b)).", "outer(Y).");
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "pair".to_string(),
                arity: 2
            }
        );
    }

    #[test]
    fn rule_with_deeply_nested_struct() {
        let query = compile_program(
            "wrap(X) :- data(X). data(node(leaf(a), leaf(b))).",
            "wrap(node(leaf(a), leaf(b))).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_shared_variable_in_body() {
        let query = compile_program("same(X) :- eq(X, X). eq(a, a).", "same(a).");
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_shared_variable_propagation() {
        // Query: connect(a, Z). Z=Y(0) が "c" に束縛
        let query = compile_program(
            "connect(X, Z) :- link(X, Y), link(Y, Z). link(a, b). link(b, c).",
            "connect(a, Z).",
        );
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "c".to_string(),
                arity: 0
            }
        );
    }

    #[test]
    fn rule_three_args() {
        // Query: triple(A, B, C). A=Y(0), B=Y(1), C=Y(2)
        let query = compile_program(
            "triple(X, Y, Z) :- first(X), second(Y), third(Z). first(a). second(b). third(c).",
            "triple(A, B, C).",
        );
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "a".to_string(),
                arity: 0
            }
        );
        assert_eq!(
            get_y_cell(&mut state, 1),
            Cell::Struct {
                functor: "b".to_string(),
                arity: 0
            }
        );
        assert_eq!(
            get_y_cell(&mut state, 2),
            Cell::Struct {
                functor: "c".to_string(),
                arity: 0
            }
        );
    }

    // バックトラックが必要（同じ述語に複数のファクトがある）
    #[test]
    #[ignore]
    fn rule_mixed_with_facts() {
        let query = compile_program(
            "animal(dog). animal(cat). is_pet(X) :- animal(X).",
            "is_pet(dog).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_head_with_struct() {
        let query = compile_program(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(pair(a, b)).",
        );
        let result = execute_instructions(&query);
        assert!(result.is_ok());
    }

    #[test]
    fn rule_head_with_struct_var_query() {
        // Query: make_pair(P). P=Y(0) が pair(a, b) に束縛
        let query = compile_program(
            "make_pair(pair(X, Y)) :- left(X), right(Y). left(a). right(b).",
            "make_pair(P).",
        );
        let mut state = execute_instructions(&query).unwrap();
        assert_eq!(
            get_y_cell(&mut state, 0),
            Cell::Struct {
                functor: "pair".to_string(),
                arity: 2
            }
        );
    }
}
