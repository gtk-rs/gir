use env::Env;
use library;
use nameutil;
use super::trampolines;
use super::imports::Imports;

#[derive(Debug)]
pub struct Info {
    pub connect_name: String,
    pub signal_name: String,
    pub trampoline_name: Option<String>, //TODO: remove Option
}

pub fn analyze(env: &Env, signals: &[library::Signal], type_tid: library::TypeId,
               in_trait: bool, trampolines: &mut trampolines::Trampolines,
               imports: &mut Imports) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let info = analyze_signal(env, signal, type_tid, in_trait, trampolines, imports);
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal(env: &Env, signal: &library::Signal, type_tid: library::TypeId,
                  in_trait: bool, trampolines: &mut trampolines::Trampolines,
                  imports: &mut Imports) -> Option<Info> {
    let mut used_types: Vec<String> = Vec::with_capacity(4);

    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline_name = trampolines::analyze(env, signal, type_tid, in_trait, trampolines,
                                               &mut used_types);

    if trampoline_name.is_some() {
        for s in used_types {
            if let Some(i) = s.find("::") {
                imports.add(&s[..i], None);
            } else {
                imports.add(&s, None);
            }
        }

        if in_trait {
            imports.add("Object", None);
        }

        imports.add("glib::signal::connect", None);
        imports.add("std::mem::transmute", None);
    }

    let info = Info {
        connect_name: connect_name,
        signal_name: signal.name.clone(),
        trampoline_name: trampoline_name,
    };
    Some(info)
}
