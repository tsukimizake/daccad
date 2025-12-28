use std::ops::{Index, IndexMut};
use std::rc::Rc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellIndex(usize);
impl CellIndex {
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    pub(crate) const EMPTY: CellIndex = CellIndex(0);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Empty {
        id: CellIndex,
    },
    Var {
        id: CellIndex,
        name: String,
    },
    Struct {
        id: CellIndex,
        functor: String,
        arity: usize,
        children: Vec<CellIndex>,
    },
}

impl Cell {
    pub fn id(&self) -> CellIndex {
        match self {
            Cell::Empty { id } | Cell::Var { id, .. } | Cell::Struct { id, .. } => *id,
        }
    }
}

pub struct CellHeap {
    cells: Vec<Rc<Cell>>,
}

impl CellHeap {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(16);
        // 0は常にEmptyセル
        cells.push(Rc::new(Cell::Empty {
            id: CellIndex::EMPTY,
        }));
        Self { cells }
    }

    fn next_index(&self) -> CellIndex {
        CellIndex(self.cells.len())
    }

    pub fn insert_empty(&mut self) -> CellIndex {
        let id = self.next_index();
        self.cells.push(Rc::new(Cell::Empty { id }));
        id
    }

    pub fn insert_var(&mut self, name: impl Into<String>) -> CellIndex {
        let id = self.next_index();
        self.cells.push(Rc::new(Cell::Var {
            id,
            name: name.into(),
        }));
        id
    }

    pub fn insert_struct(&mut self, functor: String, children: Vec<CellIndex>) -> CellIndex {
        let id = self.next_index();
        let arity = children.len();
        self.cells.push(Rc::new(Cell::Struct {
            id,
            functor,
            arity,
            children,
        }));
        id
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
