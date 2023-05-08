use crate::{
    index::{AnyIndex, IndexType},
    table::TableType,
    Index,
};

pub(crate) enum ConstraintInner<T: TableType + 'static> {
    /// Unique constraint based on index.
    Unique(Box<dyn AnyIndex<T>>),
    /// Constraint based on closure check.
    Check(fn(&T) -> bool),
}

pub struct Constraint<T: TableType + 'static>(pub(crate) ConstraintInner<T>);

impl<T: TableType> Constraint<T> {
    pub fn unique<I: IndexType + 'static>(index: &Index<T, I>) -> Self {
        Self(ConstraintInner::Unique(Box::new(index.clone())))
    }

    pub fn check(check: fn(&T) -> bool) -> Self {
        Self(ConstraintInner::Check(check))
    }
}
