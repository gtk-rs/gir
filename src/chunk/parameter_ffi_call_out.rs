use library;

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
}

impl<'a> From<&'a library::Parameter> for Parameter {
    fn from(orig: &'a library::Parameter) -> Parameter {
        Parameter {
            name: orig.name.clone(),
            typ: orig.typ,
            transfer: orig.transfer,
        }
    }
}
