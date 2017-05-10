use config;
use config::gobjects::GObject;
use env::Env;
use library;
use nameutil;
use super::trampolines;
use super::imports::Imports;
use traits::*;
use version::Version;

#[derive(Debug)]
pub struct Info {
    pub connect_name: String,
    pub signal_name: String,
    pub trampoline_name: Result<String, Vec<String>>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc_hidden: bool,
}

pub fn analyze(env: &Env, signals: &[library::Signal], type_tid: library::TypeId,
               in_trait: bool, trampolines: &mut trampolines::Trampolines,
               obj: &GObject, imports: &mut Imports) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let configured_signals = obj.signals.matched(&signal.name);
        if configured_signals.iter().any(|f| f.ignore) {
            continue;
        }
        if env.is_totally_deprecated(signal.deprecated_version) {
            continue;
        }

        let info = analyze_signal(env, signal, type_tid, in_trait, &configured_signals, trampolines, imports);
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal(env: &Env, signal: &library::Signal, type_tid: library::TypeId,
                  in_trait: bool, configured_signals: &[&config::signals::Signal],
                  trampolines: &mut trampolines::Trampolines,
                  imports: &mut Imports) -> Option<Info> {
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let version = configured_signals.iter().filter_map(|f| f.version).min()
        .or(signal.version);
    let deprecated_version = signal.deprecated_version;
    let doc_hidden = configured_signals.iter().any(|f| f.doc_hidden);

    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline_name = trampolines::analyze(env, signal, type_tid, in_trait, configured_signals,
                                               trampolines, &mut used_types, version);

    if trampoline_name.is_ok() {
        imports.add_used_types(&used_types, version);
        if in_trait {
            imports.add("glib", version);
            imports.add("glib::object::Downcast", version);
        }
        imports.add("glib::signal::connect", version);
        imports.add("std::mem::transmute", version);
        imports.add("std::boxed::Box as Box_", version);
        imports.add("glib_ffi", version);
    }

    let info = Info {
        connect_name: connect_name,
        signal_name: signal.name.clone(),
        trampoline_name: trampoline_name,
        version: version,
        deprecated_version: deprecated_version,
        doc_hidden: doc_hidden,
    };
    Some(info)
}
