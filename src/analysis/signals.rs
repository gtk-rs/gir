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
    pub is_detailed: bool,
    pub generate_doc: bool,
}

pub fn analyze(
    env: &Env,
    signals: &[library::Signal],
    type_tid: library::TypeId,
    in_trait: bool,
    is_fundamental: bool,
    obj: &GObject,
    imports: &mut Imports,
) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let configured_signals = obj.signals.matched(&signal.name);
        if !configured_signals.iter().all(|f| f.status.need_generate()) {
            continue;
        }
        if env.is_totally_deprecated(Some(type_tid.ns_id), signal.deprecated_version) {
            continue;
        }

        let info = analyze_signal(
            env,
            signal,
            type_tid,
            in_trait,
            is_fundamental,
            &configured_signals,
            obj,
            imports,
        );
        sns.push(info);
    }

    sns
}

fn analyze_signal(
    env: &Env,
    signal: &library::Signal,
    type_tid: library::TypeId,
    in_trait: bool,
    is_fundamental: bool,
    configured_signals: &[&config::signals::Signal],
    obj: &GObject,
    imports: &mut Imports,
) -> Info {
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let version = configured_signals
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(signal.version);
    let deprecated_version = signal.deprecated_version;
    let doc_hidden = configured_signals.iter().any(|f| f.doc_hidden);

    let imports = &mut imports.with_defaults(version, &None);
    imports.add("glib::translate::*");

    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
    let trampoline = trampolines::analyze(
        env,
        signal,
        type_tid,
        in_trait,
        is_fundamental,
        configured_signals,
        obj,
        &mut used_types,
        version,
    );

    let action_emit_name = if signal.is_action {
        imports.add("glib::prelude::*");
        Some(format!("emit_{}", nameutil::signal_to_snake(&signal.name)))
    } else {
        None
    };

    if trampoline.is_ok() {
        imports.add_used_types(&used_types);
        imports.add("glib::prelude::*");
        imports.add("glib::signal::{connect_raw, SignalHandlerId}");
        imports.add("std::mem::transmute");
        imports.add("std::boxed::Box as Box_");
    }
    let generate_doc = configured_signals.iter().all(|f| f.generate_doc);

    Info {
        connect_name,
        signal_name: signal.name.clone(),
        trampoline,
        action_emit_name,
        version,
        deprecated_version,
        doc_hidden,
        is_detailed: signal.is_detailed,
        generate_doc,
    }
}
