use config::gobjects::*;
use library::*;

#[derive(Debug)]
pub struct StatusedTypeId {
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}
