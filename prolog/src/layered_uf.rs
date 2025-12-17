mod cell_store;
mod uf_core;

use cell_store::CellStore;
use std::rc::Rc;
use uf_core::UfCore;

pub use cell_store::Cell;

// Cell登録をCellStoreに委譲し、UF本体はUfCoreに分離したラッパー
pub struct LayeredUf {
    store: CellStore,
    uf: UfCore,
}

#[allow(dead_code)]
impl LayeredUf {
    pub fn new() -> LayeredUf {
        LayeredUf {
            store: CellStore::new(),
            uf: UfCore::new(),
        }
    }

    pub fn register_empty(&mut self) -> usize {
        let id = self.uf.register_node();
        self.store.insert_empty(id);
        id
    }

    pub fn register_var(&mut self, name: impl Into<String>) -> usize {
        let id = self.uf.register_node();
        self.store.insert_var(id, name);
        id
    }

    pub fn register_struct(&mut self, functor: impl Into<String>, children: Vec<usize>) -> usize {
        let id = self.uf.register_node();
        self.store.insert_struct(id, functor, children);
        id
    }

    pub fn value(&self, id: usize) -> Rc<Cell> {
        self.store.value(id)
    }

    pub fn find(&mut self, id: usize) -> Rc<Cell> {
        let root_id = self.uf.find_root(id);
        self.store.value(root_id)
    }

    pub fn union(&mut self, parent_id: usize, child_id: usize) {
        self.uf.union(parent_id, child_id);
    }

    pub fn push_choicepoint(&mut self) {
        self.uf.push_choicepoint();
    }

    pub fn pop_choicepoint(&mut self) {
        self.uf.pop_choicepoint();
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
