use std::{collections::HashMap, hash::Hash, rc::Rc};

#[allow(dead_code)]
// wam互換prologバイトコードインタプリタ用の、union-findにスタックを加えたデータ構造
// choicepointでVecに新しいunion-find層をpushし、バックトラック時にpopする
// unionやpath compactionは最新の層でのみ行われ、それより下の層は不変データ構造として扱われる
pub struct StackedUf<T: Eq + Hash> {
    // TODO HashMapだと定数倍の性能に限界があるため、変数の出現順に連番を振り、
    // {
    //   name_table: HashMap<usize, T>,
    //   state: Vec<usize>
    // }
    // のような内部構造にすることを考えている
    state: Vec<HashMap<Rc<T>, Rc<T>>>,
}

#[allow(dead_code)]
impl<T: Eq + Hash> StackedUf<T> {
    pub fn new() -> StackedUf<T> {
        let mut s = StackedUf {
            state: Vec::with_capacity(10),
        };
        s.state.push(HashMap::new());
        s
    }

    fn root(&mut self, x: &Rc<T>) -> Rc<T> {
        let mut current = Rc::clone(x);
        let mut path = Vec::with_capacity(10);
        path.push(Rc::clone(x));

        let (head, tail) = split_stack(&mut self.state);

        loop {
            let next = match head.get(&current) {
                Some(parent) => Rc::clone(parent),
                None => visit_tail_root(tail, &current),
            };

            if Rc::ptr_eq(&current, &next) {
                for node in path {
                    head.insert(node, Rc::clone(&current));
                }
                return current;
            }
            path.push(Rc::clone(&next));
            current = next;
        }
    }

    pub fn union(&mut self, l: &Rc<T>, r: &Rc<T>) {
        let lroot = self.root(l);
        let rroot = self.root(r);
        if Rc::ptr_eq(&lroot, &rroot) {
            return;
        }

        let (head, _tail) = split_stack(&mut self.state);
        head.insert(lroot, rroot.clone());
        head.insert(l.clone(), rroot);
    }

    pub fn find(&mut self, from: &Rc<T>) -> Rc<T> {
        self.root(from)
    }

    pub fn push_choicepoint(&mut self) {
        self.state.push(HashMap::new());
    }
    pub fn pop_choicepoint(&mut self) {
        if self.state.len() > 1 {
            self.state.pop();
        } else {
            panic!();
        }
    }
}

fn split_stack<T: Eq + Hash>(
    state: &mut [HashMap<Rc<T>, Rc<T>>],
) -> (&mut HashMap<Rc<T>, Rc<T>>, &[HashMap<Rc<T>, Rc<T>>]) {
    if let Some((head, tail)) = state.split_last_mut() {
        (head, tail)
    } else {
        panic!()
    }
}

fn visit_tail_root<'a, T: Eq + Hash>(tail: &'a [HashMap<Rc<T>, Rc<T>>], x: &Rc<T>) -> Rc<T> {
    for level in tail.iter().rev() {
        match level.get(x) {
            Some(xparent) => {
                return xparent.clone();
            }
            None => continue,
        }
    }
    Rc::clone(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_unconnected() {
        let mut uf = StackedUf::new();
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
        let mut uf = StackedUf::new();
        let a = Rc::new(1);
        let b = Rc::new(2);

        uf.union(&a, &b);

        let root_a = uf.find(&a);
        let root_b = uf.find(&b);

        assert!(Rc::ptr_eq(&root_a, &root_b));
    }

    #[test]
    fn test_transitive_union() {
        let mut uf = StackedUf::new();
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
        let mut uf: StackedUf<i32> = StackedUf::new();
        uf.pop_choicepoint();
    }

    #[test]
    fn test_backtrack_undoes_union() {
        let mut uf = StackedUf::new();
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
        let mut uf: StackedUf<i32> = StackedUf::new();
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
        let mut uf = StackedUf::new();
        let x = Rc::new("x".to_string());
        let y = Rc::new("y".to_string());

        uf.union(&x, &y);

        let root_x = uf.find(&x);
        let root_y = uf.find(&y);

        assert!(Rc::ptr_eq(&root_x, &root_y));
    }
}
