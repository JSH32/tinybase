/// Modifying operation applied to table.
#[derive(PartialEq, Clone)]
pub enum Operation {
    Insert,
    Delete,
    Update,
}
