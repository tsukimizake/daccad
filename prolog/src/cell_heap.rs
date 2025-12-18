use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Empty {
        id: usize,
    },
    Var {
        id: usize,
        name: String,
    },
    Struct {
        id: usize,
        functor: String,
        arity: usize,
        children: Vec<usize>,
    },
}

impl Cell {
    pub fn id(&self) -> usize {
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

    fn next_id(&self) -> usize {
        self.cells.len()
    }

    pub fn insert_empty(&mut self) -> usize {
        let id = self.next_id();
        self.cells.push(Rc::new(Cell::Empty { id }));
        id
    }

    pub fn insert_var(&mut self, name: impl Into<String>) -> usize {
        let id = self.next_id();
        self.cells.push(Rc::new(Cell::Var {
            id,
            name: name.into(),
        }));
        id
    }

    pub fn insert_struct(&mut self, functor: String, children: Vec<usize>) -> usize {
        let id = self.next_id();
        let arity = children.len();
        self.cells.push(Rc::new(Cell::Struct {
            id,
            functor,
            arity,
            children,
        }));
        id
    }

    pub fn value(&self, id: usize) -> Rc<Cell> {
        self.cells[id].clone()
    }
}
