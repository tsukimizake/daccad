use std::{collections::HashMap, hash::Hash, rc::Rc};

#[allow(dead_code)]
// wam互換prologバイトコードインタプリタ用の、union-findにスタックを加えたデータ構造
// choicepointでVecに新しいunion-find層をpushし、バックトラック時にpopする
// unionやpath compactionは最新の層でのみ行われ、それより下の層は不変データ構造として扱われる
pub struct LayeredUf<T: Eq + Hash> {
    name_table: Vec<Rc<T>>,
    name_layers: Vec<HashMap<Rc<T>, usize>>,
    // layers are sparse overlays: each layer holds slots that existed when it was
    // pushed (UNSET means “see older layer”). Newer nodes only extend the head layer
    layers: Vec<Layer>,
}

const UNSET: usize = usize::MAX;

struct Layer {
    parent: Vec<usize>,
    size: Vec<usize>,
}

#[allow(dead_code)]
impl<T: Eq + Hash> LayeredUf<T> {
    pub fn new() -> LayeredUf<T> {
        LayeredUf {
            name_table: Vec::with_capacity(16),
            name_layers: vec![HashMap::with_capacity(16)],
            layers: vec![Layer {
                parent: Vec::with_capacity(16),
                size: Vec::with_capacity(16),
            }],
        }
    }

    fn lookup_id(&mut self, node: &Rc<T>) -> Option<usize> {
        let len = self.name_layers.len();
        for idx in (0..len).rev() {
            if let Some(&id) = self.name_layers[idx].get(node) {
                if idx + 1 != len {
                    let head = self.name_layers.last_mut().unwrap();
                    head.insert(Rc::clone(node), id);
                }
                return Some(id);
            }
        }
        None
    }

    fn ensure_id(&mut self, node: &Rc<T>) -> usize {
        if let Some(id) = self.lookup_id(node) {
            return id;
        }

        let id = self.name_table.len();
        self.name_table.push(Rc::clone(node));
        self.extend_head_layer();
        let head = self.layers.last_mut().unwrap();
        head.parent[id] = id;
        head.size[id] = 1;
        self.name_layers
            .last_mut()
            .unwrap()
            .insert(Rc::clone(node), id);
        id
    }

    fn extend_head_layer(&mut self) {
        let new_len = self.name_table.len();
        if let Some(head) = self.layers.last_mut() {
            let missing = new_len.saturating_sub(head.parent.len());
            if missing > 0 {
                head.parent.extend(std::iter::repeat(UNSET).take(missing));
                head.size.extend(std::iter::repeat(UNSET).take(missing));
            }
        }
    }

    fn get_parent(&self, node: usize) -> usize {
        for layer in self.layers.iter().rev() {
            if node < layer.parent.len() {
                let p = layer.parent[node];
                if p != UNSET {
                    return p;
                }
            }
        }
        node
    }

    fn get_size(&self, node: usize) -> usize {
        for layer in self.layers.iter().rev() {
            if node < layer.size.len() {
                let s = layer.size[node];
                if s != UNSET {
                    return s;
                }
            }
        }
        1
    }

    fn find_root_id(&mut self, mut idx: usize) -> usize {
        let mut path = Vec::with_capacity(8);
        loop {
            let parent = self.get_parent(idx);
            if parent == idx {
                break;
            }
            path.push(idx);
            idx = parent;
        }
        let root = idx;

        let head = self.layers.last_mut().unwrap();
        for node in path {
            head.parent[node] = root;
        }
        root
    }

    pub fn find(&mut self, x: &Rc<T>) -> Rc<T> {
        let idx = self.ensure_id(x);
        let root_id = self.find_root_id(idx);
        self.name_table[root_id].clone()
    }

    pub fn union(&mut self, l: &Rc<T>, r: &Rc<T>) {
        let l_id = self.ensure_id(l);
        let r_id = self.ensure_id(r);
        let mut l_root = self.find_root_id(l_id);
        let mut r_root = self.find_root_id(r_id);

        if l_root == r_root {
            return;
        }

        let l_size = self.get_size(l_root);
        let r_size = self.get_size(r_root);
        if l_size < r_size {
            std::mem::swap(&mut l_root, &mut r_root);
        }

        let head = self.layers.last_mut().unwrap();
        head.parent[r_root] = l_root;
        head.size[l_root] = l_size + r_size;
    }

    pub fn push_choicepoint(&mut self) {
        let len = self.name_table.len();
        let mut parent = Vec::with_capacity(len);
        parent.resize(len, UNSET);
        let mut size = Vec::with_capacity(len);
        size.resize(len, UNSET);
        self.layers.push(Layer { parent, size });
        self.name_layers.push(HashMap::with_capacity(8));
    }
    pub fn pop_choicepoint(&mut self) {
        if self.layers.len() <= 1 {
            panic!();
        }

        let layer = self.layers.pop().unwrap();
        self.name_layers.pop();
        drop(layer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_unconnected() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);

        let root_a = uf.find(&a);
        let root_b = uf.find(&b);

        assert!(Rc::ptr_eq(&root_a, &a));
        assert!(Rc::ptr_eq(&root_b, &b));
        assert!(!Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_union_and_find() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);

        uf.union(&a, &b);

        let root_a = uf.find(&a);
        let root_b = uf.find(&b);

        assert!(Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_transitive_union() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);
        let c = Rc::new(3);

        uf.union(&a, &b);
        uf.union(&b, &c);

        let root_a = uf.find(&a);
        let root_c = uf.find(&c);

        assert!(Rc::ptr_eq(&root_a, &root_c));
    }

    #[test]
    #[should_panic]
    fn test_pop_choicepoint_empty_panics() {
        let mut uf: LayeredUf<i32> = LayeredUf::new();
        uf.pop_choicepoint();
    }

    #[test]
    fn test_backtrack_undoes_union() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);

        uf.union(&a, &b);
        let root_before = uf.find(&a);

        uf.push_choicepoint();
        let c = Rc::new(3);
        uf.union(&a, &c);

        let root_a = uf.find(&a);
        let root_c = uf.find(&c);
        assert!(Rc::ptr_eq(&root_a, &root_c));

        uf.pop_choicepoint();

        let root_after = uf.find(&a);
        let root_c_after = uf.find(&c);

        assert!(Rc::ptr_eq(&root_after, &root_before));
        assert!(!Rc::ptr_eq(&root_after, &root_c_after));
    }

    #[test]
    fn test_choicepoint_isolation() {
        let mut uf: LayeredUf<i32> = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);

        uf.union(&a, &b);

        uf.push_choicepoint();
        let c = Rc::new(3);
        let d = Rc::new(4);
        uf.union(&c, &d);

        let root_a = uf.find(&a);
        let root_b = uf.find(&b);
        let root_c = uf.find(&c);
        let root_d = uf.find(&d);

        assert!(Rc::ptr_eq(&root_a, &root_b));
        assert!(Rc::ptr_eq(&root_c, &root_d));
        assert!(!Rc::ptr_eq(&root_a, &root_c));

        uf.pop_choicepoint();

        let root_a_after = uf.find(&a);
        let root_b_after = uf.find(&b);
        assert!(Rc::ptr_eq(&root_a_after, &root_b_after));
    }

    #[test]
    fn test_string_values() {
        let mut uf = LayeredUf::new();
        let x = Rc::new("x".to_string());
        let y = Rc::new("y".to_string());

        uf.union(&x, &y);

        let root_x = uf.find(&x);
        let root_y = uf.find(&y);

        assert!(Rc::ptr_eq(&root_x, &root_y));
    }
}
