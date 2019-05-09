use crate::config::gobjects::*;
use crate::library::*;

#[derive(Debug, Clone)]
pub struct StatusedTypeId {
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}
