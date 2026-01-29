mod internal_vecs;

use crate::cell_heap::CellIndex;
use internal_vecs::{
    AllLayers, AllLayersIndex, CurrentLayerParents, LocalParentIndex, OldLayersParents, Parents,
};

pub use internal_vecs::{GlobalParentIndex, Parent};

// parent配列はglobal indexとlocal indexの2種類のインデックスを持つ
// local indexはそのlayer内でのindexで、2レイヤ目以降も0から始まる
// レイヤ0は親参照も同じなのでこんな感じの普通UF
// [(0,0), (0,0), (2,2)]
// find_rootでレイヤ内参照をまず優先して見てpath compactionして、そののちrootを見に行って自分だけcompactionしてという動作をし、レイヤ1以降はこのようになる
// [(0,0), (0,0), (2,2), | (0, 0), (3, 0), (5, 2), (2,3) | (6,0) | (8,0)]
// レイヤをまたいで大きい方向に参照することはない（古いレイヤから新しいレイヤへの参照は発生しない）
#[derive(Debug)]
pub struct LayeredUf {
    // union-findの親ノードを示す配列 いつものやつ
    // 自分より新しいレイヤのノードは参照しない制約
    // rootedは下位レイヤにまたがる親、localは現レイヤ内の親
    // root探索時はlocalのみ探索してからrootedを探索することで一貫性を担保する
    parent: Parents,
    // 各レイヤーの開始インデックス
    // top layerは最後尾で、それ以前のレイヤーは不変データ構造として扱う
    // layer_indexは半開区間で、layer i は [start_i, start_{i+1}) を意味する
    layer_index: AllLayers,
}

impl LayeredUf {
    pub fn new() -> Self {
        let mut layer_index = AllLayers(Vec::with_capacity(100));
        layer_index.0.push(GlobalParentIndex(0));
        layer_index.push(GlobalParentIndex::layer_end_sentry());
        Self {
            parent: Parents(Vec::with_capacity(1000)),
            layer_index,
        }
    }

    #[allow(unused)]
    pub fn debug_dump(&self) {
        use std::fmt::Write;

        let mut out = String::new();
        writeln!(out, "LayeredUf").unwrap();
        writeln!(out, "parents_len: {}", self.parent.len()).unwrap();

        write!(out, "layer_index(raw): [").unwrap();
        for (i, idx) in self.layer_index.iter().enumerate() {
            if i > 0 {
                write!(out, ", ").unwrap();
            }
            if idx.is_empty() {
                write!(out, "MAX").unwrap();
            } else {
                write!(out, "{}", idx.0).unwrap();
            }
        }
        writeln!(out, "]").unwrap();

        writeln!(out, "parents:").unwrap();
        for (global, parent) in self.parent.iter().enumerate() {
            writeln!(
                out,
                "  [{}] local={} rooted={}  cell={:?}",
                global, parent.local.0, parent.rooted.0, parent.cell
            )
            .unwrap();
        }

        println!("{}", out);
    }

    pub fn alloc_node(&mut self) -> GlobalParentIndex {
        let global_id = GlobalParentIndex(self.parent.len());
        let top_layer_start = self.layer_index.get_top();
        let local_id = LocalParentIndex::from_global_index(global_id, top_layer_start.0);
        self.parent.push(Parent::new(global_id, local_id));
        global_id
    }

    #[allow(unused)]
    pub fn alloc_node_with_parent(&mut self, old_node: GlobalParentIndex) -> GlobalParentIndex {
        let global_id = GlobalParentIndex(self.parent.len());
        let top_layer_start = self.layer_index.get_top();

        debug_assert!(
            old_node < *top_layer_start,
            "old_node must be in an old layer"
        );

        let local_id = LocalParentIndex::from_global_index(global_id, top_layer_start.0);
        self.parent.push(Parent::new(old_node, local_id));
        global_id
    }

    pub fn set_cell(&mut self, id: GlobalParentIndex, cell: CellIndex) {
        let (old_layers, mut current_layer, is_top_layer) = self.split_layers(id);
        debug_assert!(is_top_layer, "set_cell called on non-top layer");
        let local_root_idx =
            find_local_root(id, old_layers.0.len(), &mut current_layer, is_top_layer);
        current_layer[local_root_idx].cell = cell;
    }

    // (nodeを含むlayerより下のlayers, nodeを含むlayer, is_top_layer)を返す
    fn split_layers(
        &mut self,
        node: GlobalParentIndex,
    ) -> (OldLayersParents<'_>, CurrentLayerParents<'_>, bool) {
        // todo bisect
        let current_layer_beg_idx: AllLayersIndex = self
            .layer_index
            .iter()
            .rposition(|layer_beg| *layer_beg <= node)
            .map(AllLayersIndex)
            .unwrap();
        let current_layer_end_idx = current_layer_beg_idx + 1;
        let current_layer_beg = self.layer_index[current_layer_beg_idx];
        let current_layer_end = self.layer_index[current_layer_end_idx];
        let is_top_layer = current_layer_end_idx.0 == self.layer_index.len() - 1;

        let (old_layers, newer_layers) = self.parent.split_at_mut(current_layer_beg);
        if is_top_layer {
            (
                OldLayersParents(old_layers),
                CurrentLayerParents(newer_layers),
                true,
            )
        } else {
            let current_layer_len = current_layer_end.0 - current_layer_beg.0;
            let (current_layer, _rest_layers) = newer_layers.split_at_mut(current_layer_len);
            (
                OldLayersParents(old_layers),
                CurrentLayerParents(current_layer),
                false,
            )
        }
    }

    // nodeのrootを返す. 注意点として、local_rootにcellが設定されている場合はそれを優先して返す
    pub fn find_root<'a>(&'a mut self, id: GlobalParentIndex) -> &'a Parent {
        let (old_layers, mut current_layer, is_top_layer) = self.split_layers(id);
        let root_idx = find_root_impl(&old_layers, &mut current_layer, is_top_layer, id);
        &self.parent[root_idx]
    }

    // 常に l <- r の向きでリンク（レイヤをまたいで大きい方向に参照することはない）
    // l_id, r_idはともにtop layerに存在することが前提
    pub fn union(&mut self, l_id: GlobalParentIndex, r_id: GlobalParentIndex) -> bool {
        let (old_layers, mut current_layer, is_top_layer) = self.split_layers(l_id);

        debug_assert!(is_top_layer, "union called on non-top layer(l)");
        debug_assert!(
            old_layers.0.len() <= r_id.0,
            "union called on non-top layer(r)"
        );

        let l_localroot = find_local_root(l_id, old_layers.0.len(), &mut current_layer, true);
        let r_localroot = find_local_root(r_id, old_layers.0.len(), &mut current_layer, true);

        if l_localroot == r_localroot {
            return true;
        }

        // 常に l <- r の向きでリンク (lがrを指す、rがルートになる)
        current_layer[l_localroot].local = r_localroot;

        true
    }

    #[allow(unused)]
    pub fn push_choicepoint(&mut self) {
        self.layer_index.0.pop();
        self.layer_index
            .0
            .push(GlobalParentIndex(self.parent.0.len()));
        self.layer_index
            .0
            .push(GlobalParentIndex::layer_end_sentry());
    }

    #[allow(unused)]
    pub fn pop_choicepoint(&mut self) {
        if self.layer_index.len() <= 2 {
            panic!("pop on layer_index.len() <= 2")
        }
        // remove sentry
        self.layer_index.0.pop();

        let layer_start = self
            .layer_index
            .pop()
            .expect("no choicepoint to pop in LayeredUf");
        self.parent.0.truncate(layer_start.0);

        // set sentry
        self.layer_index
            .0
            .push(GlobalParentIndex::layer_end_sentry());
    }
}

// 渡ってくるnodeはalloc_node済みであることが前提
fn find_local_root(
    node: GlobalParentIndex,
    old_layers_len: usize,
    current_layer: &mut CurrentLayerParents<'_>,
    is_top_layer: bool,
) -> LocalParentIndex {
    // top layer時のみpath compactionする
    if is_top_layer {
        let mut path = Vec::with_capacity(8);
        let mut current = LocalParentIndex::from_global_index(node, old_layers_len);
        let mut next = current_layer[current].local;
        while current != next {
            path.push(current);
            current = next;
            next = current_layer[current].local;
        }
        let root = current;

        path.iter().for_each(|p| {
            current_layer[*p].local = root;
        });
        root
    } else {
        let mut current = LocalParentIndex::from_global_index(node, old_layers_len);
        let mut next = current_layer[current].local;
        while current != next {
            current = next;
            next = current_layer[current].local;
        }
        current
    }
}

fn find_root_impl(
    old_layers: &OldLayersParents<'_>,
    current_layer: &mut CurrentLayerParents<'_>,
    is_top_layer: bool,
    node: GlobalParentIndex,
) -> GlobalParentIndex {
    let old_layers_len = old_layers.0.len();
    let local_root_idx = find_local_root(node, old_layers_len, current_layer, is_top_layer);
    let local_root = &current_layer[local_root_idx];

    // そのlayerでcellが更新されている場合はそれを返す
    if !local_root.cell.is_empty() {
        return GlobalParentIndex::from_local_index(local_root_idx, old_layers_len);
    }

    // local_rootがglobal_rootの場合普通に返す
    if GlobalParentIndex::from_local_index(local_root.local, old_layers_len) == local_root.rooted {
        return local_root.rooted;
    }

    // これ以降はglobal rootはold_layersにあるはず
    debug_assert!(local_root.rooted.0 < old_layers_len);

    let rooted = current_layer[local_root_idx].rooted;
    // 型合わせのために一回アクセスしているのが汚い
    let global_root = find_root_old_layers(old_layers, rooted);
    // local rootのみglobal indexもpath compactionする
    if is_top_layer {
        current_layer[local_root_idx].rooted = global_root;
    }
    global_root
}

fn find_root_old_layers(
    old_layers: &OldLayersParents<'_>,
    node: GlobalParentIndex,
) -> GlobalParentIndex {
    let mut current = node;
    let mut next = old_layers[current].rooted; // 古いレイヤなのでrootedのみ見る

    while next != current {
        current = next;
        next = old_layers[current].rooted;
    }

    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell_heap::CellHeap;

    #[test]
    fn split_layers_simple() {
        let mut uf = LayeredUf::new();
        let id = uf.alloc_node();
        assert_eq!(id, GlobalParentIndex(0));
        let root = uf.find_root(id);
        assert_eq!(*root, Parent::new(id, LocalParentIndex(0)),);
    }

    #[test]
    fn split_layers_empty_top() {
        let mut uf = LayeredUf::new();
        let id0 = uf.alloc_node();
        uf.push_choicepoint();

        let (old_layers, current_layer, is_top_layer) = uf.split_layers(id0);
        assert_eq!(old_layers.0.len(), 0);
        assert_eq!(current_layer.0.len(), 1);
        assert_eq!(is_top_layer, false);
    }

    #[test]
    fn split_layers_boundary_indices() {
        let mut uf = LayeredUf::new();
        let id0 = uf.alloc_node();
        let id1 = uf.alloc_node();
        uf.push_choicepoint();
        let id2 = uf.alloc_node();
        uf.push_choicepoint();
        let id3 = uf.alloc_node();

        {
            let (old_layers, current_layer, is_top_layer) = uf.split_layers(id0);
            assert_eq!(old_layers.0.len(), 0);
            assert_eq!(current_layer.0.len(), 2);
            assert_eq!(is_top_layer, false);
        }
        {
            let (old_layers, current_layer, is_top_layer) = uf.split_layers(id1);
            assert_eq!(old_layers.0.len(), 0);
            assert_eq!(current_layer.0.len(), 2);
            assert_eq!(is_top_layer, false);
        }
        {
            let (old_layers, current_layer, is_top_layer) = uf.split_layers(id2);
            assert_eq!(old_layers.0.len(), 2);
            assert_eq!(current_layer.0.len(), 1);
            assert_eq!(is_top_layer, false);
        }
        {
            let (old_layers, current_layer, is_top_layer) = uf.split_layers(id3);
            assert_eq!(old_layers.0.len(), 3);
            assert_eq!(current_layer.0.len(), 1);
            assert_eq!(is_top_layer, true);
        }
    }

    #[test]
    fn find_root_uses_old_root_cell() {
        let mut uf = LayeredUf::new();
        let mut heap = CellHeap::new();
        let base = uf.alloc_node();
        let base_cell = heap.insert_var("base".to_string());
        uf.set_cell(base, base_cell);

        uf.push_choicepoint();
        let child = uf.alloc_node_with_parent(base);

        let root = uf.find_root(child);
        assert_eq!(root.cell, base_cell);
    }

    #[test]
    fn find_root_uses_local_cell_over_old_root() {
        let mut uf = LayeredUf::new();
        let mut heap = CellHeap::new();
        let base = uf.alloc_node();
        let base_cell = heap.insert_var("base".to_string());
        uf.set_cell(base, base_cell);

        uf.push_choicepoint();
        let child = uf.alloc_node_with_parent(base);
        let child_cell = heap.insert_var("child".to_string());
        uf.set_cell(child, child_cell);

        let root = uf.find_root(child);
        assert_eq!(root.cell, child_cell);
    }

    #[test]
    fn union_preserves_cell_from_non_root() {
        let mut uf = LayeredUf::new();
        let mut heap = CellHeap::new();
        let left = uf.alloc_node();
        let right = uf.alloc_node();
        let right_cell = heap.insert_var("right".to_string());
        uf.set_cell(right, right_cell);
        uf.debug_dump();

        uf.union(left, right);

        let root = uf.find_root(left);
        assert_eq!(root.cell, right_cell);
    }

    #[test]
    fn top_layer_empty_does_not_path_compress_old_layers() {
        let mut uf = LayeredUf::new();
        let id0 = uf.alloc_node();
        let id1 = uf.alloc_node();
        let id2 = uf.alloc_node();

        uf.parent[id2].local = LocalParentIndex(1);
        uf.parent[id1].local = LocalParentIndex(0);
        uf.parent[id0].local = LocalParentIndex(0);

        uf.push_choicepoint();

        let _ = uf.find_root(id2);

        assert_eq!(uf.parent[id2].local, LocalParentIndex(1));
    }
    #[test]
    fn split_layers_multiple_layers() {
        let mut uf = LayeredUf::new();
        let id1 = uf.alloc_node();
        uf.push_choicepoint();
        let id2 = uf.alloc_node();
        assert_eq!(id1, GlobalParentIndex(0));
        assert_eq!(id2, GlobalParentIndex(1));
        let root1 = uf.find_root(id1);
        assert_eq!(*root1, Parent::new(id1, LocalParentIndex(0)),);
        let root2 = uf.find_root(id2);
        assert_eq!(*root2, Parent::new(id2, LocalParentIndex(0)),);
    }
}
