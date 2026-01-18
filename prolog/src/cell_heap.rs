use crate::compiler_bytecode::VarName;
use std::ops::{Index, IndexMut};
use std::rc::Rc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellIndex(usize);
impl CellIndex {
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    pub(crate) const EMPTY: CellIndex = CellIndex(0);

    pub fn is_var(&self, heap: &CellHeap) -> bool {
        matches!(&*heap[*self], Cell::Var { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Var { name: VarName },
    VarRef { name: VarName, ref_index: CellIndex },
    Struct { functor: String, arity: usize },
}

impl Cell {}

pub struct CellHeap {
    cells: Vec<Rc<Cell>>,
}

impl CellHeap {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(16);
        // 0は常にEmptyセル
        cells.push(Rc::new(Cell::Empty {}));
        Self { cells }
    }

    fn next_index(&self) -> CellIndex {
        CellIndex(self.cells.len())
    }

    pub fn insert_var(&mut self, name: VarName) -> CellIndex {
        let id = self.next_index();
        self.cells.push(Rc::new(Cell::Var { name }));
        id
    }

    pub fn insert_struct(&mut self, functor: &String, arity: usize) -> CellIndex {
        let id = self.next_index();
        self.cells.push(Rc::new(Cell::Struct {
            functor: functor.clone(),
            arity,
        }));
        id
    }

    pub fn set_ref(&mut self, from_id: CellIndex, to_id: CellIndex) {
        if let Cell::Var { name } = &*self.cells[from_id.0] {
            self.cells[from_id.0] = Rc::new(Cell::VarRef {
                name: name.clone(),
                ref_index: to_id,
            });
        } else {
            panic!("Attempted to set reference on a non-variable cell");
        }
    }

    pub fn value(&self, id: CellIndex) -> Rc<Cell> {
        self.cells[id.0].clone()
    }
}

impl Index<CellIndex> for CellHeap {
    type Output = Rc<Cell>;

    fn index(&self, index: CellIndex) -> &Self::Output {
        &self.cells[index.0]
    }
}

impl IndexMut<CellIndex> for CellHeap {
    fn index_mut(&mut self, index: CellIndex) -> &mut Self::Output {
        &mut self.cells[index.0]
    }
}
