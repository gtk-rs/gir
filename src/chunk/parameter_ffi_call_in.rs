use library;

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub typ: library::TypeId,
    pub instance_parameter: bool,
    pub direction: library::ParameterDirection,
    pub transfer: library::Transfer,
    pub nullable: library::Nullable,
}

impl<'a> From<&'a library::Parameter> for Parameter {
    fn from(orig: &'a library::Parameter) -> Parameter {
        Parameter {
            name: orig.name.clone(),
            typ: orig.typ,
            instance_parameter: orig.instance_parameter,
            direction: orig.direction,
            transfer: orig.transfer,
            nullable: orig.nullable,
        }
    }
}
