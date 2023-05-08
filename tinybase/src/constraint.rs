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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{result::TinyBaseError, Table, TinyBase};

    #[test]
    fn table_constraint() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the constraint
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        // Add unique constraint with created index
        assert!(table.constraint(Constraint::unique(&index)).is_ok());

        // Add check constraint with condition
        assert!(table
            .constraint(Constraint::check(|value: &String| value.len() >= 5))
            .is_ok());

        table.insert("greater".to_owned()).unwrap();

        // Unique constraint.
        assert!(matches!(
            table.insert("greater".to_owned()),
            Err(TinyBaseError::Exists { .. })
        ));

        // Check constraint.
        assert!(matches!(
            table.insert("less".to_owned()),
            Err(TinyBaseError::Condition)
        ));
    }
}
