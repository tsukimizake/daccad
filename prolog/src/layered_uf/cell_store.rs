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

pub struct CellStore {
    cells: Vec<Rc<Cell>>,
}

impl CellStore {
    pub fn new() -> Self {
        Self {
            cells: Vec::with_capacity(16),
        }
    }

    pub fn insert_empty(&mut self, id: usize) {
        self.assert_contiguous(id);
        self.cells.push(Rc::new(Cell::Empty { id }));
    }

    pub fn insert_var(&mut self, id: usize, name: impl Into<String>) {
        self.assert_contiguous(id);
        self.cells
            .push(Rc::new(Cell::Var { id, name: name.into() }));
    }

    pub fn insert_struct(&mut self, id: usize, functor: impl Into<String>, children: Vec<usize>) {
        self.assert_contiguous(id);
        let arity = children.len();
        self.cells.push(Rc::new(Cell::Struct {
            id,
            functor: functor.into(),
            arity,
            children,
        }));
    }

    pub fn value(&self, id: usize) -> Rc<Cell> {
        self.cells[id].clone()
    }

    fn assert_contiguous(&self, id: usize) {
        if id != self.cells.len() {
            panic!("cell id must be contiguous");
        }
    }
}
