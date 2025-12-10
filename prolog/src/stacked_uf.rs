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
