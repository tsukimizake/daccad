//   bugs
//   - DONE High: split_layers のレイヤー判定が壊れていて find+< で最初のレイヤーしか選ばず境界ノードも誤分類
//   - DONE layer_index が空/トップ層だと layer_index[..] 参照で panic します。 src/layered_uf.rs:210 src/layered_uf.rs:214 src/layered_uf.rs:215
//  - DONE High: split_layers の newer_layers.split_at_mut にグローバル index をそのまま渡しており、層開始が 0 以外だと current/rest の境界がずれます。src/layered_uf.rs:218
//   - High: find_root_impl の LocalParentIndex::from_global_index(local_root.rooted, old_layers_len) は rooted が旧層だと underflow します（global→local 変換の向きが逆）。src/layered_uf.rs:346
//   - High: find_root_impl/find_root_old_layers のキャッシュ経路が不整合で、node 由来 cache を使ったまま old_layers[node] を参照するため現層ノードで OOB、かつ cache-hit 分岐が todo!() のままです。src/layered_uf.rs:353 src/layered_uf.rs:366 src/layered_uf.rs:367
//   - Low: レイヤー分割や push/pop、跨層 root 解決を検証する単体テストがなく、回帰しやすいです。src/layered_uf.rs:172

use std::{
    cmp,
    ops::{Add, Deref, DerefMut, Index, IndexMut, Sub},
};

use crate::cell_heap::CellIndex;

// parent配列はglobal indexとlocal indexの2種類のインデックスを持つ
// local indexはそのlayer内でのindexで、2レイヤ目以降も0から始まる
// レイヤ0は親参照も同じなのでこんな感じの普通UF
// [(0,0), (0,0), (2,2)]
// find_rootでレイヤ内参照をまず優先して見てpath compactionして、そののちrootを見に行って自分だけcompactionしてという動作をし、レイヤ1以降はこのようになる
// [(0,0), (0,0), (2,2), | (0, 0), (3, 0), (5, 2), (2,3) | (6,0) | (8,0)]
// また、キャッシュなしだと上記の場合に7にアクセスすると7,6,2と毎回たどり、path compactionが効かないためキャッシュを導入している
// cell_storeなど外のレイヤでバックトラック後に初めて使われた変数かどうかをチェックできれば最新レイヤに参照をpushすることでキャッシュは不要になる
// 参照は必ずインデックスが小さいものへと参照し、最もインデックスが小さいものがレイヤ内の代表元となる
pub struct LayeredUf {
    // union-findの親ノードを示す配列 いつものやつ
    // 自分より新しいレイヤのノードは参照しない制約
    // rootedは下位レイヤにまたがる親、localは現レイヤ内の親
    // root探索時はlocalのみ探索してからrootedを探索することで一貫性を担保する
    parent: Parents,
    // 各レイヤーの開始インデックス
    // top layerは最後尾で、それ以前のレイヤーは不変データ構造として扱う
    layer_index: AllLayers,
    // キャッシュの世代管理用 pop_choicepoint時にインクリメントされる
    epoch: u32,
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

    // top layer以外ではpath compactionが起きない弱点を補うためのキャッシュ
    // epochをチェックすることによってbacktrack後はstaleとして扱われる
    rooted_cache: GlobalParentIndex,
    cache_epoch: u32,
    // rootの場合にcellへの参照(id)を持つ
    // local rootの場合もそのlayerで書き込まれた場合に持つ場合がある
    cell: Option<CellIndex>,
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
struct RestLayersParents<'a>(&'a [Parent]);

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
            epoch: 0,
        }
    }

    #[allow(unused)]
    pub fn debug_dump(&self) -> String {
        use std::fmt::Write;

        let mut out = String::new();
        writeln!(out, "LayeredUf");
        writeln!(out, "epoch: {}", self.epoch);
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
            let cell = match parent.cell {
                Some(cell) => usize::from(cell).to_string(),
                None => "None".to_string(),
            };
            writeln!(
                out,
                "  [{}] local={} rooted={} rooted_cache={} cache_epoch={} cell={}",
                global,
                parent.local.0,
                parent.rooted.0,
                parent.rooted_cache.0,
                parent.cache_epoch,
                cell
            );
        }

        out
    }

    #[allow(unused)]
    pub fn register_node(&mut self) -> GlobalParentIndex {
        let global_id = GlobalParentIndex(self.parent.len());
        let top_layer_start = self.layer_index.get_top();
        let local_id = LocalParentIndex::from_global_index(global_id, top_layer_start.0);
        self.parent.push(Parent {
            rooted: global_id,
            local: local_id,
            rooted_cache: global_id,
            cache_epoch: self.epoch,
            cell: None,
        });
        global_id
    }

    // (nodeを含むlayerより下のlayers, nodeを含むlayer, 残りのlayers)を返す
    fn split_layers<'a>(
        &mut self,
        node: GlobalParentIndex,
    ) -> (
        OldLayersParents<'_>,
        CurrentLayerParents<'_>,
        RestLayersParents<'_>,
    ) {
        // todo bisect
        let current_layer_end_idx: AllLayersIndex = self
            .layer_index
            .iter()
            .enumerate()
            .find(|(_, layer_beg)| node < **layer_beg)
            .map(|(idx, _)| AllLayersIndex(idx))
            .unwrap();
        let current_layer_beg_idx = current_layer_end_idx - 1;
        let current_layer_beg = self.layer_index[current_layer_beg_idx];
        let current_layer_end = self.layer_index[current_layer_end_idx];
        let all_parent_len = GlobalParentIndex(self.parent.len());

        let (old_layers, newer_layers) = self.parent.split_at_mut(current_layer_beg);
        if current_layer_end < all_parent_len {
            let (current_layer, rest_layers) = newer_layers.split_at_mut(current_layer_end.0);
            (
                OldLayersParents(old_layers),
                CurrentLayerParents(current_layer),
                RestLayersParents(rest_layers),
            )
        } else {
            (
                OldLayersParents(old_layers),
                CurrentLayerParents(newer_layers),
                RestLayersParents(&[]),
            )
        }
    }

    #[allow(unused)]
    pub fn find_root<'a>(&'a mut self, id: GlobalParentIndex) -> &'a Parent {
        let epoch = self.epoch;
        let (mut old_layers, mut current_layer, rest_layers) = self.split_layers(id);
        let is_top_layer = rest_layers.0.is_empty();
        let root_idx = find_root_impl(epoch, &mut old_layers, &mut current_layer, is_top_layer, id);
        &self.parent[root_idx]
    }

    // 必ずindexが大きいものから小さいものを参照
    #[allow(unused)]
    pub fn union(&mut self, l_id: GlobalParentIndex, r_id: GlobalParentIndex) {
        let epoch = self.epoch;
        let (mut old_layers, mut current_layer, rest_layers) = self.split_layers(l_id);
        let is_top_layer = rest_layers.0.is_empty();
        let l_root = find_root_impl(
            epoch,
            &mut old_layers,
            &mut current_layer,
            is_top_layer,
            l_id,
        );
        let r_root = find_root_impl(
            epoch,
            &mut old_layers,
            &mut current_layer,
            is_top_layer,
            r_id,
        );
        todo!()
    }

    // pub fn union(&mut self, l_id: usize, r_id: usize) {
    //     //let parent_root = self.find_root(parent_id);
    //     //let old_child_root = self.find_root(child_id);
    //     let l_root = todo!();
    //     let r_root = todo!();

    //     if l_root == r_root {
    //         return;
    //     }

    //     // 現レイヤのlocalを更新し、下位レイヤからも辿れるようrootedも更新
    //     self.parent[r_id].local = l_root;
    //     self.parent[r_id].rooted = l_root;
    //     // キャッシュも更新
    //     self.parent[r_id].rooted_cache = l_root;
    //     self.parent[r_id].cache_epoch = self.epoch;
    // }

    #[allow(unused)]
    pub fn push_choicepoint(&mut self) {
        self.layer_index.0.split_off(2);
        self.layer_index
            .0
            .push(GlobalParentIndex(self.parent.0.len() - 1));
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
        self.epoch += 1;
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

// 渡ってくるnodeはregister_node済みであることが前提
fn find_root_impl(
    global_epoch: u32,
    old_layers: &mut OldLayersParents<'_>,
    current_layer: &mut CurrentLayerParents<'_>,
    is_top_layer: bool,
    node: GlobalParentIndex,
) -> GlobalParentIndex {
    let old_layers_len = old_layers.0.len();
    let local_root_idx = find_local_root(node, old_layers_len, current_layer, is_top_layer);
    let local_root = &current_layer[local_root_idx];

    // そのlayerでcellが更新されている場合はそれを返す
    if local_root.cell.is_some() {
        return GlobalParentIndex::from_local_index(local_root_idx, old_layers_len);
    }

    // local_rootがglobal_rootの場合普通に返す
    // これ以降はglobal rootはold_layersにあるはず
    if local_root.local == LocalParentIndex::from_global_index(local_root.rooted, old_layers_len) {
        return local_root.rooted;
    }
    let old_layer_index = node;
    return find_root_old_layers(
        global_epoch,
        old_layers,
        current_layer[LocalParentIndex::from_global_index(old_layer_index, old_layers_len)]
            .rooted_cache,
        old_layer_index,
    );
}

fn find_root_old_layers(
    global_epoch: u32,
    old_layers: &mut OldLayersParents<'_>,
    rooted_cache: GlobalParentIndex,
    node: GlobalParentIndex,
) -> GlobalParentIndex {
    // cacheがfreshならcacheを利用
    if old_layers[node].cache_epoch == global_epoch {
        todo!()
    }
    let mut current = rooted_cache;
    let mut next = old_layers[current].rooted; // 古いレイヤなのでrootedのみ見る

    while next != current {
        current = next;
        next = old_layers[current].rooted;
    }

    // TODO 結果を見て呼び出し元でキャッシュ更新
    // self.parent[node].rooted_cache = current;
    // self.parent[node].cache_epoch = self.epoch;
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_layers_panics_without_choicepoint() {
        let mut uf = LayeredUf::new();
        let id = uf.register_node();
        assert_eq!(id, GlobalParentIndex(0));
        let root = uf.find_root(id);
        assert_eq!(
            *root,
            Parent {
                rooted: id,
                local: LocalParentIndex(0),
                rooted_cache: id,
                cache_epoch: 0,
                cell: None,
            }
        );
    }
}
