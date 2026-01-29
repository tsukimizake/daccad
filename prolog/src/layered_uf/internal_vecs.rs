use std::ops::{Add, Deref, DerefMut, Index, IndexMut, Sub};

use crate::cell_heap::CellIndex;

// ==================== Index types ====================

/// Parents全てに対するインデックス型
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GlobalParentIndex(pub(crate) usize);

impl GlobalParentIndex {
    /// 無効値（番兵）
    pub(crate) const EMPTY: GlobalParentIndex = GlobalParentIndex(usize::MAX);

    pub(crate) fn from_local_index(
        index: LocalParentIndex,
        old_layers_len: usize,
    ) -> GlobalParentIndex {
        GlobalParentIndex(index.0 + old_layers_len)
    }

    pub(crate) fn layer_end_sentry() -> Self {
        GlobalParentIndex(usize::MAX)
    }

    /// 無効値かどうかを判定
    #[inline]
    pub(crate) fn is_empty(self) -> bool {
        self.0 == usize::MAX
    }

    /// オフセットを加算した新しいGlobalParentIndexを返す
    pub(crate) fn offset(base: GlobalParentIndex, offset: usize) -> GlobalParentIndex {
        GlobalParentIndex(base.0 + offset)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct LocalParentIndex(pub(crate) usize);

impl LocalParentIndex {
    pub(crate) fn from_global_index(
        index: GlobalParentIndex,
        old_layers_len: usize,
    ) -> LocalParentIndex {
        LocalParentIndex(index.0 - old_layers_len)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct AllLayersIndex(pub(crate) usize);

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

// ==================== Wrapper types ====================

/// 本来cellを持つかどうかでenumにしたいところだが性能のためにこの形
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parent {
    /// layerをまたぐ親への参照
    /// top layer以外では不変性を保つため更新されない
    pub(crate) rooted: GlobalParentIndex,
    /// layer内での親への参照
    /// top layer以外では不変性を保つため更新されない
    /// rooted, local共にindexが小さい物へと参照する.結果として代表元は最もindexが小さいものとなる
    pub(crate) local: LocalParentIndex,

    /// cellへの参照(id)を持つ. 代表元以外の場合は意味を持たない
    /// local rootの場合もそのlayerで書き込まれた場合はそこがそのlayerでのcellとなる
    pub(crate) cell: CellIndex,
}

impl Parent {
    pub(crate) fn new(rooted: GlobalParentIndex, local: LocalParentIndex) -> Self {
        Self {
            rooted,
            local,
            cell: CellIndex::EMPTY,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Parents(pub(crate) Vec<Parent>);

impl Parents {
    pub(crate) fn split_at_mut(
        &mut self,
        mid: GlobalParentIndex,
    ) -> (&mut [Parent], &mut [Parent]) {
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

pub(crate) struct OldLayersParents<'a>(pub(crate) &'a [Parent]);

impl<'a> Index<GlobalParentIndex> for OldLayersParents<'a> {
    type Output = Parent;
    fn index(&self, i: GlobalParentIndex) -> &Self::Output {
        &self.0[i.0]
    }
}

pub(crate) struct CurrentLayerParents<'a>(pub(crate) &'a mut [Parent]);

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

#[derive(Debug)]
pub(crate) struct AllLayers(pub(crate) Vec<GlobalParentIndex>);

impl AllLayers {
    pub(crate) fn get_top(&self) -> &GlobalParentIndex {
        // top layerとsentryは必ず入っているので常にlenは2以上
        // lastに入っているsentryを飛ばして返すため-2
        &self.0[self.0.len() - 2]
    }
}

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
