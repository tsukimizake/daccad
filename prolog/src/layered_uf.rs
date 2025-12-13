use std::rc::Rc;

const NONE: usize = usize::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Empty { id: usize },
    Var { id: usize, name: String },
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
            Cell::Empty { id }
            | Cell::Var { id, .. }
            | Cell::Struct { id, .. } => *id,
        }
    }
}

// 単一のバージョン配列に変更を積み、depthで有効範囲を切り替えるunion-find。
// Cell生成もここで行い、id払い出しとUF登録を一体化させる。
pub struct LayeredUf {
    cells: Vec<Rc<Cell>>,
    versions: Vec<Version>,
    current: Vec<usize>,
    next_id: usize,
    depth: usize,
}

struct Version {
    // which logical node this version belongs to
    owner: usize,
    // parent id at the time this version was created
    parent: usize,
    // previous version of the same owner
    prev: usize,
    // choicepoint depth when this version was created
    depth: usize,
}

#[allow(dead_code)]
impl LayeredUf {
    pub fn new() -> LayeredUf {
        LayeredUf {
            cells: Vec::with_capacity(16),
            versions: Vec::with_capacity(16),
            current: Vec::with_capacity(16),
            next_id: 0,
            depth: 0,
        }
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn insert_cell(&mut self, cell: Cell) -> usize {
        let id = cell.id();
        if id != self.cells.len() {
            panic!("cell id must be contiguous");
        }

        let version_idx = self.versions.len();
        self.cells.push(Rc::new(cell));
        self.versions.push(Version {
            owner: id,
            parent: id,
            prev: NONE,
            depth: self.depth,
        });
        self.current.push(version_idx);
        id
    }

    pub fn register_empty(&mut self) -> usize {
        let id = self.alloc_id();
        self.insert_cell(Cell::Empty { id })
    }

    pub fn register_var(&mut self, name: impl Into<String>) -> usize {
        let id = self.alloc_id();
        self.insert_cell(Cell::Var { id, name: name.into() })
    }

    pub fn register_struct(&mut self, functor: impl Into<String>, children: Vec<usize>) -> usize {
        let id = self.alloc_id();
        let arity = children.len();
        self.insert_cell(Cell::Struct {
            id,
            functor: functor.into(),
            arity,
            children,
        })
    }

    pub fn value(&self, id: usize) -> Rc<Cell> {
        self.cells[id].clone()
    }

    fn resolve_version(&mut self, node_id: usize) -> usize {
        let mut idx = self.current[node_id];
        while self.versions[idx].depth > self.depth {
            let prev = self.versions[idx].prev;
            if prev == NONE {
                break;
            }
            idx = prev;
        }
        self.current[node_id] = idx;
        idx
    }

    fn find_root_id(&mut self, node: usize) -> usize {
        let mut version_idx = self.resolve_version(node);
        let mut path = Vec::with_capacity(8);

        loop {
            let version = &self.versions[version_idx];
            if version.parent == version.owner {
                break;
            }
            path.push(version.owner);
            version_idx = self.resolve_version(version.parent);
        }

        let root_owner = self.versions[version_idx].owner;
        for n in path {
            let prev = self.current[n];
            let new_idx = self.versions.len();
            self.versions.push(Version {
                owner: n,
                parent: root_owner,
                prev,
                depth: self.depth,
            });
            self.current[n] = new_idx;
        }
        root_owner
    }

    pub fn find(&mut self, id: usize) -> Rc<Cell> {
        let root_id = self.find_root_id(id);
        self.cells[root_id].clone()
    }

    pub fn union(&mut self, parent_id: usize, child_id: usize) {
        let parent_root = self.find_root_id(parent_id);
        let child_root = self.find_root_id(child_id);

        if parent_root == child_root {
            return;
        }

        let child_prev = self.current[child_root];
        let parent_prev = self.current[parent_root];

        let child_idx = self.versions.len();
        self.versions.push(Version {
            owner: child_root,
            parent: parent_root,
            prev: child_prev,
            depth: self.depth,
        });
        self.current[child_root] = child_idx;

        let new_root_idx = self.versions.len();
        self.versions.push(Version {
            owner: parent_root,
            parent: parent_root,
            prev: parent_prev,
            depth: self.depth,
        });
        self.current[parent_root] = new_root_idx;
    }

    pub fn push_choicepoint(&mut self) {
        self.depth += 1;
    }

    pub fn pop_choicepoint(&mut self) {
        if self.depth == 0 {
            panic!();
        }

        self.depth -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_unconnected() {
        let mut uf = LayeredUf::new();
        let a_id = uf.register_var("a");
        let b_id = uf.register_var("b");

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);

        assert!(matches!(*root_a, Cell::Var { id, .. } if id == a_id));
        assert!(matches!(*root_b, Cell::Var { id, .. } if id == b_id));
        assert!(!Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_union_and_find() {
        let mut uf = LayeredUf::new();
        let a_id = uf.register_var("a");
        let b_id = uf.register_var("b");

        uf.union(a_id, b_id);

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);

        assert!(Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_transitive_union() {
        let mut uf = LayeredUf::new();
        let a_id = uf.register_var("a");
        let b_id = uf.register_var("b");
        let c_id = uf.register_var("c");

        uf.union(a_id, b_id);
        uf.union(b_id, c_id);

        let root_a = uf.find(a_id);
        let root_c = uf.find(c_id);

        assert!(Rc::ptr_eq(&root_a, &root_c));
    }

    #[test]
    #[should_panic]
    fn test_pop_choicepoint_empty_panics() {
        let mut uf: LayeredUf = LayeredUf::new();
        uf.pop_choicepoint();
    }

    #[test]
    fn test_backtrack_undoes_union() {
        let mut uf = LayeredUf::new();
        let a_id = uf.register_var("a");
        let b_id = uf.register_var("b");

        uf.union(a_id, b_id);
        let root_before = uf.find(a_id);

        uf.push_choicepoint();
        let c_id = uf.register_var("c");
        uf.union(a_id, c_id);

        let root_a = uf.find(a_id);
        let root_c = uf.find(c_id);
        assert!(Rc::ptr_eq(&root_a, &root_c));

        uf.pop_choicepoint();

        let root_after = uf.find(a_id);

        assert!(Rc::ptr_eq(&root_after, &root_before));
    }

    #[test]
    fn test_choicepoint_isolation() {
        let mut uf: LayeredUf = LayeredUf::new();
        let a_id = uf.register_var("a");
        let b_id = uf.register_var("b");

        uf.union(a_id, b_id);

        uf.push_choicepoint();
        let c_id = uf.register_var("c");
        let d_id = uf.register_var("d");
        uf.union(c_id, d_id);

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);
        let root_c = uf.find(c_id);
        let root_d = uf.find(d_id);

        assert!(Rc::ptr_eq(&root_a, &root_b));
        assert!(Rc::ptr_eq(&root_c, &root_d));
        assert!(!Rc::ptr_eq(&root_a, &root_c));

        uf.pop_choicepoint();

        let root_a_after = uf.find(a_id);
        let root_b_after = uf.find(b_id);
        assert!(Rc::ptr_eq(&root_a_after, &root_b_after));
    }

    #[test]
    fn test_string_values() {
        let mut uf = LayeredUf::new();
        let x_id = uf.register_var("x".to_string());
        let y_id = uf.register_var("y".to_string());

        uf.union(x_id, y_id);

        let root_x = uf.find(x_id);
        let root_y = uf.find(y_id);

        assert!(Rc::ptr_eq(&root_x, &root_y));
    }
}
