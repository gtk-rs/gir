use env::Env;
use library;
use nameutil::signal_to_snake;
use super::conversion_type::ConversionType;

#[derive(Debug)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Vec<library::Parameter>,
    pub ret: library::Parameter,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(env: &Env, signal: &library::Signal, trampolines: &mut Trampolines) -> Option<String> {
    if !can_generate(env, signal) {
        return None;
    }

    let name = format!("{}_trampoline", signal_to_snake(&signal.name));

    let trampoline = Trampoline {
        name: name.clone(),
        parameters: signal.parameters.clone(),
        ret: signal.ret.clone(),
    };
    trampolines.push(trampoline);
    Some(name)
}

fn can_generate(env: &Env, signal: &library::Signal) -> bool {
    if signal.ret.typ != Default::default() &&
        ConversionType::of(&env.library, signal.ret.typ) == ConversionType::Unknown {
            return false;
        }
    for par in &signal.parameters {
        if ConversionType::of(&env.library, par.typ) == ConversionType::Unknown {
            return false;
        }
    }
    true
}
