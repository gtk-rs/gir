use config::gobjects::*;
use library::*;

pub struct StatusedTypeId {
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}
