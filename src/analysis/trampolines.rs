
use library;

#[derive(Debug)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Vec<library::Parameter>,
    pub ret: library::Parameter,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(signal: &library::Signal, trampolines: &mut Trampolines) -> Option<String> {
    let name = generate_name(signal);
    if name.is_none() { return None }

    let trampoline = Trampoline {
        name: name.clone().unwrap(),
        parameters: signal.parameters.clone(),
        ret: signal.ret.clone(),
    };
    trampolines.push(trampoline);
    name
}

fn generate_name(signal: &library::Signal) -> Option<String> {
    let ret_name = match name_from_return_type(&signal.ret) {
        Some(name) => name,
        None => return None,
    };
    if signal.parameters.is_empty() {
        Some(format!("{}_trampoline", ret_name))
    } else {
        //TODO: trampolines with params
        None
    }
}

fn name_from_return_type(ret: &library::Parameter) -> Option<String> {
    if ret.typ == Default::default() {
        Some("void".to_owned())
    } else {
        None
    }
}
