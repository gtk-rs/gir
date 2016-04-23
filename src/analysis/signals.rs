use config::gobjects::GObject;
use config::identables::Identables;
use env::Env;
use library;
use nameutil;
use super::trampolines;
use super::imports::Imports;
use version::Version;

#[derive(Debug)]
pub struct Info<'e> {
    pub connect_name: String,
    pub signal_name: &'e str,
    pub trampoline_name: Option<String>, //TODO: remove Option
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
}

pub fn analyze<'e>(env: &'e Env, signals: &'e [library::Signal], type_tid: library::TypeId,
               in_trait: bool, trampolines: &mut trampolines::Trampolines<'e>,
               obj: &GObject, imports: &mut Imports) -> Vec<Info<'e>> {
    let mut sns = Vec::new();

    for signal in signals {
        let configured_signals = obj.signals.matched(&signal.name);
        if configured_signals.iter().any(|f| f.ignore) {
            continue;
        }

        let info = analyze_signal(env, signal, type_tid, in_trait, trampolines, imports);
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal<'e>(env: &'e Env, signal: &'e library::Signal, type_tid: library::TypeId,
                  in_trait: bool, trampolines: &mut trampolines::Trampolines<'e>,
                  imports: &mut Imports) -> Option<Info<'e>> {
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let version = signal.version;
    let deprecated_version = signal.deprecated_version;

    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline_name = trampolines::analyze(env, signal, type_tid, in_trait, trampolines,
                                               &mut used_types, version);

    if trampoline_name.is_some() {
        imports.add_used_types(&used_types, version);
        if in_trait {
            imports.add("Object", version);
            imports.add("glib::object::Downcast", version);
        }
        imports.add("glib::signal::connect", version);
        imports.add("std::mem::transmute", version);
        imports.add("std::boxed::Box as Box_", version);
        imports.add("glib_ffi::gpointer", version);
    }

    let info = Info {
        connect_name: connect_name,
        signal_name: &*signal.name,
        trampoline_name: trampoline_name,
        version: version,
        deprecated_version: deprecated_version,
    };
    Some(info)
}
