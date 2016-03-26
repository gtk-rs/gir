use env::Env;
use library;
use nameutil;
use super::trampolines;

#[derive(Debug)]
pub struct Info {
    pub connect_name: String,
    pub signal_name: String,
    pub trampoline_name: Option<String>, //TODO: remove Option
}

pub fn analyze(env: &Env, signals: &[library::Signal], type_tid: library::TypeId,
               in_trait: bool, trampolines: &mut trampolines::Trampolines) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let info = analyze_signal(env, signal, type_tid, in_trait, trampolines);
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal(env: &Env, signal: &library::Signal, type_tid: library::TypeId,
                  in_trait: bool, trampolines: &mut trampolines::Trampolines) -> Option<Info> {
    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline_name = trampolines::analyze(env, signal, type_tid, in_trait, trampolines);

    let info = Info {
        connect_name: connect_name,
        signal_name: signal.name.clone(),
        trampoline_name: trampoline_name,
    };
    Some(info)
}
