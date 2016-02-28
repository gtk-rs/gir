use env::Env;
use library;

#[derive(Debug)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Vec<library::Parameter>,
    pub ret: library::Parameter,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(env: &Env, signal: &library::Signal, trampolines: &mut Trampolines) -> Option<String> {
    let name = generate_name(env, signal, trampolines);
    if name.is_none() { return None }

    let trampoline = Trampoline {
        name: name.clone().unwrap(),
        parameters: signal.parameters.clone(),
        ret: signal.ret.clone(),
    };
    trampolines.push(trampoline);
    name
}

fn generate_name(env: &Env, signal: &library::Signal, trampolines: &mut Trampolines) -> Option<String> {
    let ret_name = match name_from_return_type(env, &signal.ret) {
        Some(name) => name,
        None => return None,
    };
    let mut name = if signal.parameters.is_empty() {
        Some(format!("{}_trampoline", ret_name))
    } else {
        //TODO: trampolines with params
        None
    };

    if name.is_none() {
        name = Some(format!("{}_unnamed_trampoline_{}", ret_name, trampolines.len() + 1));
    }
    name
}

fn name_from_return_type(env: &Env, ret: &library::Parameter) -> Option<String> {
    use library::Type::*;
    let some = |s: &str| Some(s.to_owned());
    match *env.type_(ret.typ) {
        Fundamental(fund) => {
            use library::Fundamental;
            match fund {
                Fundamental::None => some("void"),
                Fundamental::Boolean => some("bool"),
                Fundamental::Int => some("int"),
                _ => None,
            }
        }
        _ => None
    }
    /* TODO: other return types:
    ComboBox:format-entry-text => return utf8
    confirm-overwrite => return GtkFileChooserConfirmation
    create-context => return Gdk.GLContext
    create-window => return Notebook
    create-custom-widget => return GObject.Object
    Scale:format-value => return utf8
    */
}
