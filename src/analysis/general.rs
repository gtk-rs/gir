use std::borrow::Cow;

use config::gobjects::*;
use library::*;

pub struct StatusedTypeId<'e>{
    pub type_id: TypeId,
    pub name: Cow<'e, str>,
    pub status: GStatus,
}
