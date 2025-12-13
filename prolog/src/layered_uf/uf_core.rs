// レイヤ0は親参照も同じなのでこんな感じの普通UF
// [(0,0), (0,0), (2,2)]
// find_rootでレイヤ内参照をまず優先して見てpath compactionして、そののちrootを見に行って自分だけcompactionしてという動作をし、レイヤ1以降はこのようになる
// [(0,0), (0,0), (2,2), | (0, 3), (3, 3), (5, 5), (2,6) | (6,7) | (8,8)]
// note or fixme: 上記の場合に7にアクセスすると7,6,2と毎回たどり、path compactionが効かない。このような場合にtop layerに(7,9)のような参照を追加し、9でpath compactionを行えば改善する
pub struct UfCore {
    // union-findの親ノードを示す配列 いつものやつ
    // 自分より新しいレイヤのノードは参照しない制約
    // permanentは下位レイヤにまたがる親、localは現レイヤ内の親
    // root探索時はlocalのみ探索してからpermanentを探索することで一貫性を担保する
    parent: Vec<Parent>,
    // 各レイヤーの開始インデックス
    // top layerは最後尾で、それ以前のレイヤーは不変データ構造として扱う
    layer_index: Vec<usize>,
}

struct Parent {
    permanent: usize,
    local: usize,
}

impl UfCore {
    pub fn new() -> Self {
        Self {
            parent: Vec::with_capacity(1000),
            layer_index: Vec::with_capacity(100),
        }
    }

    fn top_layer_start(&self) -> usize {
        *self.layer_index.last().unwrap_or(&0)
    }

    pub fn register_node(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(Parent {
            permanent: id,
            local: id,
        });
        id
    }

    // 渡ってくるnodeはregister_node済みであることが前提
    pub fn find_root(&mut self, node: usize) -> usize {
        let mut path = Vec::with_capacity(8);

        // レイヤ判定
        let top_start = self.top_layer_start();
        let mut current = node;
        let mut parent = if current < top_start {
            self.parent[current].permanent // 古いレイヤなのでpermanentのみ見る
        } else {
            self.parent[current].local // 現レイヤ優先
        };

        // レイヤ内参照を優先しながら辿る
        while parent != current {
            if current >= top_start {
                path.push(current);
            }
            current = parent;
            parent = if current < top_start {
                self.parent[current].permanent
            } else {
                self.parent[current].local
            };
        }

        // permanent側で最終根を確認
        let mut root = current;
        let mut perm = self.parent[root].permanent;
        while perm != root {
            root = perm;
            perm = self.parent[root].permanent;
        }

        // 現レイヤのlocalに圧縮書き込み
        for n in path {
            if n >= top_start {
                self.parent[n].local = root;
            }
        }

        root
    }

    pub fn union(&mut self, parent_id: usize, child_id: usize) {
        let parent_root = self.find_root(parent_id);
        let old_child_root = self.find_root(child_id);

        if parent_root == old_child_root {
            return;
        }

        // 現レイヤのlocalを更新し、下位レイヤからも辿れるようpermanentも更新
        self.parent[child_id].local = parent_root;
        self.parent[child_id].permanent = parent_root;
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
    }
}
