use std::ops::{Add, Deref, DerefMut, Index, IndexMut, Sub};

use crate::cell_heap::CellIndex;

// parent配列はglobal indexとlocal indexの2種類のインデックスを持つ
// local indexはそのlayer内でのindexで、2レイヤ目以降も0から始まる
// レイヤ0は親参照も同じなのでこんな感じの普通UF
// [(0,0), (0,0), (2,2)]
// find_rootでレイヤ内参照をまず優先して見てpath compactionして、そののちrootを見に行って自分だけcompactionしてという動作をし、レイヤ1以降はこのようになる
// [(0,0), (0,0), (2,2), | (0, 0), (3, 0), (5, 2), (2,3) | (6,0) | (8,0)]
// 参照は必ずインデックスが小さいものへと参照し、最もインデックスが小さいものがレイヤ内の代表元となる
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

// 本来cellを持つかどうかでenumにしたいところだが性能のためにこの形
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parent {
    // layerをまたぐ親への参照
    // top layer以外では不変性を保つため更新されない
    rooted: GlobalParentIndex,
    // layer内での親への参照
    // top layer以外では不変性を保つため更新されない
    // rooted, local共にindexが小さい物へと参照する.結果として代表元は最もindexが小さいものとなる
    local: LocalParentIndex,

    // cellへの参照(id)を持つ. 代表元以外の場合は意味を持たない
    // local rootの場合もそのlayerで書き込まれた場合はそこがそのlayerでのcellとなる
    cell: CellIndex,
}

struct Parents(Vec<Parent>);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct LocalParentIndex(usize);

impl LocalParentIndex {
    fn from_global_index(index: GlobalParentIndex, old_layers_len: usize) -> LocalParentIndex {
        LocalParentIndex(index.0 - old_layers_len)
    }
}
impl<'a> Index<LocalParentIndex> for CurrentLayerParents<'a> {
    type Output = Parent;
    fn index(&self, i: LocalParentIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

impl<'a> IndexMut<LocalParentIndex> for CurrentLayerParents<'a> {
    fn index_mut(&mut self, i: LocalParentIndex) -> &mut Self::Output {
        &mut self.0[i.0]
    }
}

// Parents全てに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GlobalParentIndex(usize);

impl GlobalParentIndex {
    fn from_local_index(index: LocalParentIndex, old_layers_len: usize) -> GlobalParentIndex {
        GlobalParentIndex(index.0 + old_layers_len)
    }

    fn layer_end_sentry() -> Self {
        GlobalParentIndex(usize::max_value())
    }
}

impl Parents {
    fn split_at_mut(&mut self, mid: GlobalParentIndex) -> (&mut [Parent], &mut [Parent]) {
        let (left, right) = self.0.split_at_mut(mid.0);
        (left, right)
    }
}

impl Index<GlobalParentIndex> for Parents {
    type Output = Parent;

    fn index(&self, index: GlobalParentIndex) -> &Self::Output {
        &self.0[index.0]
    }
}

impl IndexMut<GlobalParentIndex> for Parents {
    fn index_mut(&mut self, index: GlobalParentIndex) -> &mut Self::Output {
        &mut self.0[index.0]
    }
}

impl Deref for Parents {
    type Target = Vec<Parent>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Parents {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct OldLayersParents<'a>(&'a mut [Parent]);
struct CurrentLayerParents<'a>(&'a mut [Parent]);

impl<'a> Index<GlobalParentIndex> for OldLayersParents<'a> {
    type Output = Parent;
    fn index(&self, i: GlobalParentIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

impl<'a> IndexMut<GlobalParentIndex> for OldLayersParents<'a> {
    fn index_mut(&mut self, i: GlobalParentIndex) -> &mut Self::Output {
        &mut self.0[i.0]
    }
}

struct AllLayers(Vec<GlobalParentIndex>);

impl AllLayers {
    fn get_top(&self) -> &GlobalParentIndex {
        // top layerとsentryは必ず入っているので常にlenは2以上
        // lastに入っているsentryを飛ばして返すため-2
        &self.0[self.0.len() - 2]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct AllLayersIndex(usize);

impl Index<AllLayersIndex> for AllLayers {
    type Output = GlobalParentIndex;

    fn index(&self, index: AllLayersIndex) -> &Self::Output {
        &self.0[index.0]
    }
}

impl IndexMut<AllLayersIndex> for AllLayers {
    fn index_mut(&mut self, index: AllLayersIndex) -> &mut Self::Output {
        &mut self.0[index.0]
    }
}

impl Deref for AllLayers {
    type Target = Vec<GlobalParentIndex>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllLayers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Add<usize> for AllLayersIndex {
    type Output = AllLayersIndex;

    fn add(self, rhs: usize) -> Self::Output {
        AllLayersIndex(self.0 + rhs)
    }
}

impl Sub<usize> for AllLayersIndex {
    type Output = AllLayersIndex;

    fn sub(self, rhs: usize) -> Self::Output {
        AllLayersIndex(self.0 - rhs)
    }
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
        writeln!(out, "LayeredUf");
        writeln!(out, "parents_len: {}", self.parent.len());

        write!(out, "layer_index(raw): [");
        for (i, idx) in self.layer_index.iter().enumerate() {
            if i > 0 {
                write!(out, ", ");
            }
            if idx.0 == usize::MAX {
                write!(out, "MAX");
            } else {
                write!(out, "{}", idx.0);
            }
        }
        writeln!(out, "]");

        writeln!(out, "parents:");
        for (global, parent) in self.parent.iter().enumerate() {
            writeln!(
                out,
                "  [{}] local={} rooted={}  cell={}",
                global,
                parent.local.0,
                parent.rooted.0,
                usize::from(parent.cell).to_string()
            );
        }

        println!("{}", out);
    }

    #[allow(unused)]
    pub fn register_node(&mut self) -> GlobalParentIndex {
        let global_id = GlobalParentIndex(self.parent.len());
        let top_layer_start = self.layer_index.get_top();
        let local_id = LocalParentIndex::from_global_index(global_id, top_layer_start.0);
        self.parent.push(Parent {
            rooted: global_id,
            local: local_id,
            cell: CellIndex::EMPTY,
        });
        global_id
    }

    #[allow(unused)]
    pub fn register_node_with_parent(&mut self, old_node: GlobalParentIndex) -> GlobalParentIndex {
        let global_id = GlobalParentIndex(self.parent.len());
        let top_layer_start = self.layer_index.get_top();

        debug_assert!(
            old_node < *top_layer_start,
            "old_node must be in an old layer"
        );

        let local_id = LocalParentIndex::from_global_index(global_id, top_layer_start.0);
        self.parent.push(Parent {
            rooted: old_node,
            local: local_id,
            cell: CellIndex::EMPTY,
        });
        global_id
    }

    #[allow(unused)]
    pub fn set_cell(&mut self, id: GlobalParentIndex, cell: CellIndex) {
        self.parent[id].cell = cell;
    }

    // (nodeを含むlayerより下のlayers, nodeを含むlayer, 残りのlayers)を返す
    fn split_layers<'a>(
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
    #[allow(unused)]
    pub fn find_root<'a>(&'a mut self, id: GlobalParentIndex) -> &'a Parent {
        let (mut old_layers, mut current_layer, is_top_layer) = self.split_layers(id);
        let root_idx = find_root_impl(&old_layers, &mut current_layer, is_top_layer, id);
        &self.parent[root_idx]
    }

    // 必ずindexが大きいものから小さいものを参照
    // l_id, r_idはともにtop layerに存在することが前提
    #[allow(unused)]
    pub fn union(&mut self, l_id: GlobalParentIndex, r_id: GlobalParentIndex) -> bool {
        let (mut old_layers, mut current_layer, is_top_layer) = self.split_layers(l_id);

        debug_assert!(is_top_layer, "union called on non-top layer(l)");
        debug_assert!(
            old_layers.0.len() <= r_id.0,
            "union called on non-top layer(r)"
        );

        let l_localroot = find_local_root(l_id, old_layers.0.len(), &mut current_layer, true);
        let r_localroot = find_local_root(r_id, old_layers.0.len(), &mut current_layer, true);

        if l_localroot == r_localroot {
            return true;
        } else if l_localroot < r_localroot {
            current_layer[r_localroot].local = l_localroot;
        } else {
            current_layer[l_localroot].local = r_localroot;
        }

        match (
            current_layer[l_localroot].cell,
            current_layer[r_localroot].cell,
        ) {
            (cell_l, CellIndex::EMPTY) => {}
            (CellIndex::EMPTY, cell_r) => {
                current_layer[l_localroot].cell = cell_r;
            }
            (cell_l, cell_r) => {
                return false;
            }
        }
        return true;
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
// 渡ってくるnodeはregister_node済みであることが前提
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

    // 型合わせのために一回アクセスしているのが汚い
    return find_root_old_layers(old_layers, current_layer[local_root_idx].rooted);
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
        let id = uf.register_node();
        assert_eq!(id, GlobalParentIndex(0));
        let root = uf.find_root(id);
        assert_eq!(
            *root,
            Parent {
                rooted: id,
                local: LocalParentIndex(0),
                cell: CellIndex::EMPTY,
            }
        );
    }

    #[test]
    fn split_layers_empty_top() {
        let mut uf = LayeredUf::new();
        let id0 = uf.register_node();
        uf.push_choicepoint();

        let (old_layers, current_layer, is_top_layer) = uf.split_layers(id0);
        assert_eq!(old_layers.0.len(), 0);
        assert_eq!(current_layer.0.len(), 1);
        assert_eq!(is_top_layer, false);
    }

    #[test]
    fn split_layers_boundary_indices() {
        let mut uf = LayeredUf::new();
        let id0 = uf.register_node();
        let id1 = uf.register_node();
        uf.push_choicepoint();
        let id2 = uf.register_node();
        uf.push_choicepoint();
        let id3 = uf.register_node();

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
        let base = uf.register_node();
        let base_cell = heap.insert_var("base");
        uf.set_cell(base, base_cell);

        uf.push_choicepoint();
        let child = uf.register_node_with_parent(base);

        let root = uf.find_root(child);
        assert_eq!(root.cell, base_cell);
    }

    #[test]
    fn find_root_uses_local_cell_over_old_root() {
        let mut uf = LayeredUf::new();
        let mut heap = CellHeap::new();
        let base = uf.register_node();
        let base_cell = heap.insert_var("base");
        uf.set_cell(base, base_cell);

        uf.push_choicepoint();
        let child = uf.register_node_with_parent(base);
        let child_cell = heap.insert_var("child");
        uf.set_cell(child, child_cell);

        let root = uf.find_root(child);
        assert_eq!(root.cell, child_cell);
    }

    #[test]
    fn union_preserves_cell_from_non_root() {
        let mut uf = LayeredUf::new();
        let mut heap = CellHeap::new();
        let left = uf.register_node();
        let right = uf.register_node();
        let right_cell = heap.insert_var("right");
        uf.set_cell(right, right_cell);
        uf.debug_dump();

        uf.union(left, right);

        let root = uf.find_root(left);
        assert_eq!(root.cell, right_cell);
    }

    #[test]
    fn top_layer_empty_does_not_path_compress_old_layers() {
        let mut uf = LayeredUf::new();
        let id0 = uf.register_node();
        let id1 = uf.register_node();
        let id2 = uf.register_node();

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
        let id1 = uf.register_node();
        uf.push_choicepoint();
        let id2 = uf.register_node();
        assert_eq!(id1, GlobalParentIndex(0));
        assert_eq!(id2, GlobalParentIndex(1));
        let root1 = uf.find_root(id1);
        assert_eq!(
            *root1,
            Parent {
                rooted: id1,
                local: LocalParentIndex(0),
                cell: CellIndex::EMPTY,
            }
        );
        let root2 = uf.find_root(id2);
        assert_eq!(
            *root2,
            Parent {
                rooted: id2,
                local: LocalParentIndex(0),
                cell: CellIndex::EMPTY,
            }
        );
    }
}
