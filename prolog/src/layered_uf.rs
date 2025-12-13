use std::{collections::HashMap, hash::Hash, rc::Rc};

const NONE: usize = usize::MAX;

// 単一のバージョン配列に変更を積み、各バージョンに生成時のdepthを持たせてpopをO(1)にする
pub struct LayeredUf<T: Eq + Hash> {
    name_table: Vec<Rc<T>>,
    name_layers: Vec<HashMap<Rc<T>, usize>>,
    versions: Vec<Version>,
    current: Vec<usize>,
    next_id: usize,
    depth: usize,
}

struct Version {
    owner: usize,
    parent: usize,
    prev: usize,
    depth: usize,
}

#[allow(dead_code)]
impl<T: Eq + Hash> LayeredUf<T> {
    pub fn new() -> LayeredUf<T> {
        LayeredUf {
            name_table: Vec::with_capacity(16),
            name_layers: vec![HashMap::with_capacity(16)],
            versions: Vec::with_capacity(16),
            current: Vec::with_capacity(16),
            next_id: 0,
            depth: 0,
        }
    }

    fn lookup_id(&mut self, node: &Rc<T>) -> Option<usize> {
        let len = self.name_layers.len();
        for idx in (0..len).rev() {
            if let Some(&id) = self.name_layers[idx].get(node) {
                if idx + 1 != len {
                    let head = self.name_layers.last_mut().unwrap();
                    head.insert(node.clone(), id);
                }
                return Some(id);
            }
        }
        None
    }

    pub fn id_of(&mut self, node: &Rc<T>) -> Option<usize> {
        self.lookup_id(node)
    }

    pub fn register(&mut self, node: Rc<T>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.name_table.push(node.clone());

        let version_idx = self.versions.len();
        self.versions.push(Version {
            owner: id,
            parent: id,
            prev: NONE,
            depth: self.depth,
        });
        self.current.push(version_idx);

        self.name_layers
            .last_mut()
            .unwrap()
            .insert(node.clone(), id);
        id
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

    pub fn find(&mut self, id: usize) -> Rc<T> {
        let root_id = self.find_root_id(id);
        self.name_table[root_id].clone()
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
        self.name_layers.push(HashMap::with_capacity(8));
    }

    pub fn pop_choicepoint(&mut self) {
        if self.depth == 0 {
            panic!();
        }

        self.depth -= 1;
        let layer = self.name_layers.pop();
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
        let a_id = uf.register(a.clone());
        let b_id = uf.register(b.clone());

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);

        assert!(Rc::ptr_eq(&root_a, &a));
        assert!(Rc::ptr_eq(&root_b, &b));
        assert!(!Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_union_and_find() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);
        let a_id = uf.register(a.clone());
        let b_id = uf.register(b.clone());

        uf.union(a_id, b_id);

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);

        assert!(Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_transitive_union() {
        let mut uf = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);
        let c = Rc::new(3);
        let a_id = uf.register(a.clone());
        let b_id = uf.register(b.clone());
        let c_id = uf.register(c.clone());

        uf.union(a_id, b_id);
        uf.union(b_id, c_id);

        let root_a = uf.find(a_id);
        let root_c = uf.find(c_id);

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
        let a_id = uf.register(a.clone());
        let b_id = uf.register(b.clone());

        uf.union(a_id, b_id);
        let root_before = uf.find(a_id);

        uf.push_choicepoint();
        let c = Rc::new(3);
        let c_id = uf.register(c.clone());
        uf.union(a_id, c_id);

        let root_a = uf.find(a_id);
        let root_c = uf.find(c_id);
        assert!(Rc::ptr_eq(&root_a, &root_c));

        uf.pop_choicepoint();

        let root_after = uf.find(a_id);

        assert_eq!(root_after, root_before);
    }

    #[test]
    fn test_choicepoint_isolation() {
        let mut uf: LayeredUf<i32> = LayeredUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);
        let a_id = uf.register(a.clone());
        let b_id = uf.register(b.clone());

        uf.union(a_id, b_id);

        uf.push_choicepoint();
        let c = Rc::new(3);
        let d = Rc::new(4);
        let c_id = uf.register(c.clone());
        let d_id = uf.register(d.clone());
        uf.union(c_id, d_id);

        let root_a = uf.find(a_id);
        let root_b = uf.find(b_id);
        let root_c = uf.find(c_id);
        let root_d = uf.find(d_id);

        assert_eq!(root_a, root_b);
        assert_eq!(root_c, root_d);
        assert_ne!(root_a, root_c);

        uf.pop_choicepoint();

        let root_a_after = uf.find(a_id);
        let root_b_after = uf.find(b_id);
        assert_eq!(root_a_after, root_b_after);
    }

    #[test]
    fn test_string_values() {
        let mut uf = LayeredUf::new();
        let x = Rc::new("x".to_string());
        let y = Rc::new("y".to_string());
        let x_id = uf.register(x.clone());
        let y_id = uf.register(y.clone());

        uf.union(x_id, y_id);

        let root_x = uf.find(x_id);
        let root_y = uf.find(y_id);

        assert_eq!(root_x, root_y);
    }
}
