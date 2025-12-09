use std::{collections::HashMap, hash::Hash, rc::Rc};

#[allow(dead_code)]
pub struct StackedUf<T: Eq + Hash> {
    state: Vec<HashMap<Rc<T>, Rc<T>>>,
}

#[allow(dead_code)]
impl<T: Eq + Hash> StackedUf<T> {
    pub fn new() -> StackedUf<T> {
        let mut s = StackedUf {
            state: Vec::with_capacity(1),
        };
        s.state.push(HashMap::new());
        s
    }

    fn root(&mut self, x: &Rc<T>) -> Rc<T> {
        root_impl(&mut self.state, x)
    }

    pub fn union(&mut self, l: &Rc<T>, r: &Rc<T>) {
        let (head, tail) = split_stack(&mut self.state);
    }
    pub fn find(&mut self, from: &Rc<T>) {}

    pub fn push_choicepoint() {}
    pub fn pop_choicepoint() {}
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

fn root_impl<T: Eq + Hash>(state: &mut [HashMap<Rc<T>, Rc<T>>], x: &Rc<T>) -> Rc<T> {
    let (head, tail) = split_stack(state);
    let xp = match head.get(x) {
        Some(xp) => Rc::clone(xp),
        None => {
            let tail_root = visit_tail_root(tail, x);
            head.insert(x.clone(), tail_root.clone());
            tail_root
        }
    };

    if !Rc::ptr_eq(&xp, x) {
        *head.get_mut(x).unwrap() = xp.clone();
    };
    xp
}

fn visit_tail_root<'a, T: Eq + Hash>(tail: &'a [HashMap<Rc<T>, Rc<T>>], x: &Rc<T>) -> Rc<T> {
    for level in tail.iter().rev() {
        match level.get(x) {
            Some(xparent) => {
                if Rc::ptr_eq(&xparent, x) {
                    continue;
                } else {
                    return visit_tail_root(&tail[..tail.len() - 1], &xparent);
                }
            }
            None => continue,
        }
    }
    Rc::clone(x)
}
