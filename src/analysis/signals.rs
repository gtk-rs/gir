use super::{imports::Imports, trampolines};
use crate::{
    analysis::trampolines::Trampoline,
    config::{self, gobjects::GObject},
    env::Env,
    library, nameutil,
    traits::*,
    version::Version,
};

#[derive(Debug)]
pub struct Info {
    pub connect_name: String,
    pub signal_name: String,
    pub action_emit_name: Option<String>,
    pub trampoline: Result<Trampoline, Vec<String>>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc_hidden: bool,
}

pub fn analyze(
    env: &Env,
    signals: &[library::Signal],
    type_tid: library::TypeId,
    in_trait: bool,
    obj: &GObject,
    imports: &mut Imports,
) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let configured_signals = obj.signals.matched(&signal.name);
        if configured_signals.iter().any(|f| f.ignore) {
            continue;
        }
        if env.is_totally_deprecated(signal.deprecated_version) {
            continue;
        }

        let info = analyze_signal(
            env,
            signal,
            type_tid,
            in_trait,
            &configured_signals,
            obj,
            imports,
        );
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal(
    env: &Env,
    signal: &library::Signal,
    type_tid: library::TypeId,
    in_trait: bool,
    configured_signals: &[&config::signals::Signal],
    obj: &GObject,
    imports: &mut Imports,
) -> Option<Info> {
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let version = configured_signals
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(signal.version);
    let deprecated_version = signal.deprecated_version;
    let doc_hidden = configured_signals.iter().any(|f| f.doc_hidden);

    imports.set_defaults(version, &None);

    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline = trampolines::analyze(
        env,
        signal,
        type_tid,
        in_trait,
        configured_signals,
        obj,
        &mut used_types,
        version,
    );

    let action_emit_name = if signal.is_action {
        imports.add("glib");
        imports.add("gobject_sys");
        imports.add("glib::object::ObjectExt");
        Some(format!("emit_{}", nameutil::signal_to_snake(&signal.name)))
    } else {
        None
    };

    if trampoline.is_ok() {
        imports.add_used_types(&used_types);
        if in_trait {
            imports.add("glib::object::Cast");
        } else {
            //To resolve a conflict with OSTree::ObjectType
            imports.add("glib::object::ObjectType as ObjectType_");
        }
        imports.add("glib::signal::connect_raw");
        imports.add("glib::signal::SignalHandlerId");
        imports.add("std::mem::transmute");
        imports.add("std::boxed::Box as Box_");
        imports.add("glib_sys");
    }
    imports.reset_defaults();

    let info = Info {
        connect_name,
        signal_name: signal.name.clone(),
        trampoline,
        action_emit_name,
        version,
        deprecated_version,
        doc_hidden,
    };
    Some(info)
}
