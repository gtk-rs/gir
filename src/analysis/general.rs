use gobjects::*;
use library;

pub struct StatusedTypeId{
    pub type_id: library::TypeId,
    pub name: String,
    pub status: GStatus,
}
