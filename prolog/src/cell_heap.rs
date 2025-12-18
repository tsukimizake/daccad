use std::ops::{Index, IndexMut};
use std::rc::Rc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellIndex(usize);

impl From<CellIndex> for usize {
    fn from(value: CellIndex) -> Self {
        value.0
    }
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
        Self {
            cells: Vec::with_capacity(16),
        }
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
