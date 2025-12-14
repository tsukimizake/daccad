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
    parent: Vec<Parent>,
    // 各レイヤーの開始インデックス
    // top layerは最後尾で、それ以前のレイヤーは不変データ構造として扱う
    layer_index: Vec<usize>,
    // キャッシュの世代管理用 pop_choicepoint時にインクリメントされる
    epoch: u32,
}

struct Parent {
    // layerをまたぐ親への参照
    // top layer以外では不変性を保つため更新されない
    rooted: usize,
    // layer内での親への参照
    // top layer以外では不変性を保つため更新されない
    // rooted, local共にindexが小さい物へと参照する.結果として代表元は最もindexが小さいものとなる
    local: usize,

    // top layer以外ではpath compactionが起きない弱点を補うためのキャッシュ
    // epochをチェックすることによってbacktrack後はstaleとして扱われる
    rooted_cache: usize,
    cache_epoch: u32,
}

impl UfCore {
    pub fn new() -> Self {
        Self {
            parent: Vec::with_capacity(1000),
            layer_index: Vec::with_capacity(100),
            epoch: 0,
        }
    }

    fn top_layer_start(&self) -> usize {
        *self.layer_index.last().unwrap_or(&0)
    }

    pub fn register_node(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(Parent {
            rooted: id,
            local: id,
            rooted_cache: id,
            cache_epoch: self.epoch,
        });
        id
    }

    // 渡ってくるnodeはregister_node済みであることが前提
    fn find_local_root(&mut self, node: usize) -> usize {
        let current_layer: [Parent] = {
            // todo bisect
            let current_layer_begin = self
                .layer_index
                .iter()
                .enumerate()
                .find(|(_, layer_beg)| **layer_beg < node)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let current_layer_end = current_layer_begin + 1;
            self.parent[current_layer_begin..current_layer_end]
        };
    }

    // 渡ってくるnodeはregister_node済みであることが前提
    pub fn find_root(&mut self, node: usize) -> usize {
        if self.parent[node].cache_epoch == self.epoch {
            return self.parent[node].rooted_cache;
        }

        let mut path = Vec::with_capacity(8);

        // レイヤ判定
        let top_start = self.top_layer_start();
        let mut current = node;
        let mut parent = if current < top_start {
            self.parent[current].rooted // 古いレイヤなのでrootedのみ見る
        } else {
            self.parent[current].local // 現レイヤ優先
        };

        // レイヤ内参照を優先しながら辿る
        while parent != current {
            if top_start <= current {
                path.push(current);
            }
            current = parent;
            parent = if current < top_start {
                self.parent[current].rooted
            } else {
                self.parent[current].local
            };
        }

        // 現レイヤのlocalに圧縮書き込み
        for n in path {
            if n >= top_start {
                self.parent[n].local = current;
            }
        }

        self.parent[node].rooted_cache = current;
        self.parent[node].cache_epoch = self.epoch;
        current
    }

    pub fn union(&mut self, parent_id: usize, child_id: usize) {
        let parent_root = self.find_root(parent_id);
        let old_child_root = self.find_root(child_id);

        if parent_root == old_child_root {
            return;
        }

        // 現レイヤのlocalを更新し、下位レイヤからも辿れるようrootedも更新
        self.parent[child_id].local = parent_root;
        self.parent[child_id].rooted = parent_root;
        // キャッシュも更新
        self.parent[child_id].rooted_cache = parent_root;
        self.parent[child_id].cache_epoch = self.epoch;
    }

    pub fn push_choicepoint(&mut self) {
        self.layer_index.push(self.parent.len());
    }

    pub fn pop_choicepoint(&mut self) {
        let layer_start = self
            .layer_index
            .pop()
            .expect("no choicepoint to pop in LayeredUf");
        self.parent.truncate(layer_start);
        self.epoch += 1;
    }
}
