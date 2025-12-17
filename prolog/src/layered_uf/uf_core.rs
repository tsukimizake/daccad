use std::ops::{Add, Deref, DerefMut, Index, IndexMut};

// レイヤ0は親参照も同じなのでこんな感じの普通UF
// [(0,0), (0,0), (2,2)]
// find_rootでレイヤ内参照をまず優先して見てpath compactionして、そののちrootを見に行って自分だけcompactionしてという動作をし、レイヤ1以降はこのようになる
// [(0,0), (0,0), (2,2), | (0, 3), (3, 3), (5, 5), (2,6) | (6,7) | (8,8)]
// また、キャッシュなしだと上記の場合に7にアクセスすると7,6,2と毎回たどり、path compactionが効かないためキャッシュを導入している
// cell_storeなど外のレイヤでバックトラック後に初めて使われた変数かどうかをチェックできれば最新レイヤに参照をpushすることでキャッシュは不要になる
// 参照は必ずインデックスが小さいものへと参照し、最もインデックスが小さいものがレイヤ内の代表元となる
pub struct UfCore {
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
struct Parent {
    // layerをまたぐ親への参照
    // top layer以外では不変性を保つため更新されない
    rooted: GlobalParentIndex,
    // layer内での親への参照
    // top layer以外では不変性を保つため更新されない
    // rooted, local共にindexが小さい物へと参照する.結果として代表元は最もindexが小さいものとなる
    // TODO layer local index的にして毎度の引き算を避けたい
    local: GlobalParentIndex,

    // top layer以外ではpath compactionが起きない弱点を補うためのキャッシュ
    // epochをチェックすることによってbacktrack後はstaleとして扱われる
    rooted_cache: GlobalParentIndex,
    cache_epoch: u32,
    // rootの場合にcellへの参照(id)を持つ
    // local rootの場合もそのlayerで書き込まれた場合に持つ場合がある
    cell: Option<usize>,
}

struct Parents(Vec<Parent>);

// Parents全てに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GlobalParentIndex(usize);

impl GlobalParentIndex {
    fn from_current_layer_index(
        index: CurrentLayerIndex,
        old_layers_len: usize,
    ) -> GlobalParentIndex {
        GlobalParentIndex(index.0 + old_layers_len)
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

// OldLayersに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OldLayersIndex(usize);

impl OldLayersIndex {
    fn from_global_index(parent_index: GlobalParentIndex) -> OldLayersIndex {
        OldLayersIndex(parent_index.0)
    }
}

// 現在のレイヤーに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CurrentLayerIndex(usize);

impl CurrentLayerIndex {
    fn from_global_index(
        parent_index: GlobalParentIndex,
        old_layers_len: usize,
    ) -> CurrentLayerIndex {
        CurrentLayerIndex(parent_index.0 - old_layers_len)
    }
}

// rest_layersに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RestLayersIndex(usize);
impl RestLayersIndex {
    fn from_global_index(
        parent_index: GlobalParentIndex,
        old_layers_len: usize,
        current_layer_len: usize,
    ) -> RestLayersIndex {
        RestLayersIndex(parent_index.0 - old_layers_len - current_layer_len)
    }
}
impl<'a> Index<OldLayersIndex> for OldLayersParents<'a> {
    type Output = Parent;
    fn index(&self, i: OldLayersIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

impl<'a> IndexMut<OldLayersIndex> for OldLayersParents<'a> {
    fn index_mut(&mut self, i: OldLayersIndex) -> &mut Self::Output {
        &mut self.0[i.0]
    }
}

impl<'a> Index<CurrentLayerIndex> for CurrentLayerParents<'a> {
    type Output = Parent;
    fn index(&self, i: CurrentLayerIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

impl<'a> IndexMut<CurrentLayerIndex> for CurrentLayerParents<'a> {
    fn index_mut(&mut self, i: CurrentLayerIndex) -> &mut Self::Output {
        &mut self.0[i.0]
    }
}

impl<'a> Index<RestLayersIndex> for RestLayersParents<'a> {
    type Output = Parent;
    fn index(&self, i: RestLayersIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

struct AllLayers(Vec<GlobalParentIndex>);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

impl UfCore {
    pub fn new() -> Self {
        Self {
            parent: Parents(Vec::with_capacity(1000)),
            layer_index: AllLayers(Vec::with_capacity(100)),
            epoch: 0,
        }
    }

    pub fn register_node(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(Parent {
            rooted: GlobalParentIndex(id),
            local: GlobalParentIndex(id),
            rooted_cache: GlobalParentIndex(id),
            cache_epoch: self.epoch,
            cell: None,
        });
        id
    }

    // (nodeを含むlayerより下のlayers, nodeを含むlayer, 残りのlayers)を返す
    fn split_layers(
        &mut self,
        node: GlobalParentIndex,
    ) -> (OldLayersParents, CurrentLayerParents, RestLayersParents) {
        // todo bisect
        let current_layer_beg_idx: AllLayersIndex = self
            .layer_index
            .iter()
            .enumerate()
            .find(|(_, layer_beg)| **layer_beg < node)
            .map(|(idx, _)| AllLayersIndex(idx))
            .unwrap_or(AllLayersIndex(0));
        let current_layer_end_idx = current_layer_beg_idx + 1;
        let current_layer_beg = self.layer_index[current_layer_beg_idx];
        let current_layer_end = self.layer_index[current_layer_end_idx];

        let (old_layers, newer_layers) = self.parent.split_at_mut(current_layer_beg);
        let (current_layer, rest_layers) = newer_layers.split_at_mut(current_layer_end.0);
        (
            OldLayersParents(old_layers),
            CurrentLayerParents(current_layer),
            RestLayersParents(rest_layers),
        )
    }

    pub fn find_root(&mut self, id: GlobalParentIndex) -> usize {
        let epoch = self.epoch;
        let (mut old_layers, mut current_layer, rest_layers) = self.split_layers(id);
        let root = find_root_impl(epoch, &mut old_layers, &mut current_layer, &rest_layers, id);
        if let Some(root_cell) = root.cell {
            return root_cell;
        } else {
            panic!("root doesn't have cell");
        }
    }

    pub fn union(&mut self, _l_id: usize, _r_id: usize) {
        todo!()
    }

    // 必ずindexが大きいものから小さいものを参照
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

    pub fn push_choicepoint(&mut self) {
        self.layer_index
            .0
            .push(GlobalParentIndex(self.parent.0.len()));
    }

    pub fn pop_choicepoint(&mut self) {
        let layer_start = self
            .layer_index
            .pop()
            .expect("no choicepoint to pop in LayeredUf");
        self.parent.0.truncate(layer_start.0);
        self.epoch += 1;
    }
}
// 渡ってくるnodeはregister_node済みであることが前提
fn find_local_root<'a>(
    node: GlobalParentIndex,
    old_layers_len: usize,
    current_layer: &'a mut CurrentLayerParents<'a>,
    rest_layers: &RestLayersParents<'a>,
) -> &'a Parent {
    // top layer時のみpath compactionする
    if rest_layers.0.len() == 0 {
        let mut path = Vec::with_capacity(8);
        let mut current = CurrentLayerIndex::from_global_index(node, old_layers_len);
        let mut next =
            CurrentLayerIndex::from_global_index(current_layer[current].local, old_layers_len);
        while current != next {
            path.push(current);
            current = next;
            next =
                CurrentLayerIndex::from_global_index(current_layer[current].local, old_layers_len);
        }
        let root = current;

        path.iter().for_each(|p| {
            current_layer[*p].local =
                GlobalParentIndex::from_current_layer_index(root, old_layers_len)
        });
        &current_layer[current]
    } else {
        {
            let mut current = CurrentLayerIndex::from_global_index(node, old_layers_len);
            let mut next =
                CurrentLayerIndex::from_global_index(current_layer[current].local, old_layers_len);
            while current != next {
                current = next;
                next = CurrentLayerIndex::from_global_index(
                    current_layer[current].local,
                    old_layers_len,
                );
            }
            &current_layer[current]
        }
    }
}

// 渡ってくるnodeはregister_node済みであることが前提
fn find_root_impl<'a>(
    global_epoch: u32,
    old_layers: &'a mut OldLayersParents<'a>,
    current_layer: &'a mut CurrentLayerParents<'a>,
    rest_layers: &RestLayersParents<'a>,
    node: GlobalParentIndex,
) -> &'a Parent {
    let local_root = find_local_root(node, old_layers.0.len(), current_layer, rest_layers);

    // そのlayerでcellが更新されている場合はそれを返す
    if local_root.cell.is_some() {
        return local_root;
    }

    // local_rootがglobal_rootの場合普通に返す
    // これ以降はglobal rootはold_layersにあるはず
    if local_root.local == local_root.rooted {
        return local_root;
    }
    let old_layer_index = OldLayersIndex::from_global_index(node);
    return find_root_old_layers(
        global_epoch,
        old_layers,
        old_layers[old_layer_index].rooted_cache,
        old_layer_index,
    );
}

fn find_root_old_layers<'a>(
    global_epoch: u32,
    old_layers: &'a mut OldLayersParents<'a>,
    rooted_cache: GlobalParentIndex,
    node: OldLayersIndex,
) -> &'a Parent {
    // cacheがfreshならcacheを利用
    if old_layers[node].cache_epoch == global_epoch {
        todo!()
    }
    let mut current = OldLayersIndex::from_global_index(rooted_cache);
    let mut next = OldLayersIndex::from_global_index(old_layers[current].rooted); // 古いレイヤなのでrootedのみ見る

    // レイヤ内参照を優先しながら辿る
    while next != current {
        current = next;
        next = OldLayersIndex::from_global_index(old_layers[current].rooted);
    }

    // TODO 結果を見て呼び出し元でキャッシュ更新
    // self.parent[node].rooted_cache = current;
    // self.parent[node].cache_epoch = self.epoch;
    &old_layers[current]
}
