use std::{borrow::Cow, ops::Deref};

use log::info;

use super::{
    child_properties::ChildProperties, imports::Imports, info_base::InfoBase,
    signatures::Signatures, *,
};
use crate::{
    config::gobjects::{GObject, GStatus},
    env::Env,
    library::{self, FunctionKind},
    nameutil::*,
    traits::*,
};

/// The location of an item within the object
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationInObject {
    Impl,
    VirtualExt,
    ClassExt,
    ClassExtManual,
    Ext,
    ExtManual,
    Builder,
}

#[derive(Debug, Default)]
pub struct Info {
    pub base: InfoBase,
    pub c_type: String,
    pub c_class_type: Option<String>,
    pub get_type: String,
    pub is_interface: bool,
    pub is_fundamental: bool,
    pub supertypes: Vec<general::StatusedTypeId>,
    pub final_type: bool,
    pub generate_trait: bool,
    pub trait_name: String,
    pub has_constructors: bool,
    pub has_functions: bool,
    pub virtual_methods: Vec<functions::Info>,
    pub signals: Vec<signals::Info>,
    pub notify_signals: Vec<signals::Info>,
    pub properties: Vec<properties::Property>,
    pub builder_properties: Vec<(Vec<properties::Property>, TypeId)>,
    pub builder_postprocess: Option<String>,
    pub child_properties: ChildProperties,
    pub signatures: Signatures,
    /// Specific to fundamental types
    pub ref_fn: Option<String>,
    /// Specific to fundamental types
    pub unref_fn: Option<String>,
}

impl Info {
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.trampoline.is_ok())
            || self.notify_signals.iter().any(|s| s.trampoline.is_ok())
    }

    /// Whether we should generate an impl block for this object
    /// We don't generate an impl block if the type doesn't have any of the
    /// followings:
    /// - Constructors / Functions / Builder properties (no build function)
    /// - Is a final type & doesn't have either methods / properties / child
    ///   properties / signals
    pub fn should_generate_impl_block(&self) -> bool {
        self.has_constructors
            || has_builder_properties(&self.builder_properties)
            || !(self.need_generate_trait()
                && self.methods().is_empty()
                && self.properties.is_empty()
                && self.child_properties.is_empty()
                && self.signals.is_empty())
            || self.has_functions
    }

    pub fn need_generate_inherent(&self) -> bool {
        self.has_constructors
            || self.has_functions
            || !self.need_generate_trait()
            || has_builder_properties(&self.builder_properties)
    }

    pub fn need_generate_trait(&self) -> bool {
        self.generate_trait
    }

    pub fn has_action_signals(&self) -> bool {
        self.signals.iter().any(|s| s.action_emit_name.is_some())
    }

    /// Returns the location of the function within this object
    pub fn function_location(&self, fn_info: &functions::Info) -> LocationInObject {
        if fn_info.kind == FunctionKind::ClassMethod {
            // TODO: Fix location here once we can auto generate virtual methods
            LocationInObject::ClassExt
        } else if fn_info.kind == FunctionKind::VirtualMethod {
            // TODO: Fix location here once we can auto generate virtual methods
            LocationInObject::VirtualExt
        } else if self.final_type
            || self.is_fundamental
            || matches!(
                fn_info.kind,
                FunctionKind::Constructor | FunctionKind::Function
            )
        {
            LocationInObject::Impl
        } else if fn_info.status == GStatus::Generate || self.full_name == "GObject.Object" {
            LocationInObject::Ext
        } else {
            LocationInObject::ExtManual
        }
    }

    /// Generate doc name based on function location within this object
    /// See also [`Self::function_location()`].
    /// Returns `(item/crate path including type name, just the type name)`
    pub fn generate_doc_link_info(
        &self,
        fn_info: &functions::Info,
    ) -> (Cow<'_, str>, Cow<'_, str>) {
        match self.function_location(fn_info) {
            LocationInObject::Impl => (self.name.as_str().into(), self.name.as_str().into()),
            LocationInObject::ExtManual => {
                let trait_name = format!("{}Manual", self.trait_name);
                (format!("prelude::{trait_name}").into(), trait_name.into())
            }
            LocationInObject::Ext => (
                format!("prelude::{}", self.trait_name).into(),
                self.trait_name.as_str().into(),
            ),
            LocationInObject::VirtualExt => {
                // TODO: maybe a different config for subclass trait name?
                let trait_name = format!("{}Impl", self.trait_name.trim_end_matches("Ext"));
                (
                    format!("subclass::prelude::{trait_name}").into(),
                    trait_name.into(),
                )
            }
            LocationInObject::ClassExt | LocationInObject::ClassExtManual => {
                let trait_name = format!("{}Ext", self.trait_name);
                (
                    format!("subclass::prelude::{}", trait_name).into(),
                    trait_name.into(),
                )
            }
            LocationInObject::Builder => {
                panic!("C documentation is not expected to link to builders (a Rust concept)!")
            }
        }
    }
}

impl Deref for Info {
    type Target = InfoBase;

    fn deref(&self) -> &InfoBase {
        &self.base
    }
}

pub fn has_builder_properties(builder_properties: &[(Vec<properties::Property>, TypeId)]) -> bool {
    builder_properties
        .iter()
        .map(|b| b.0.iter().len())
        .sum::<usize>()
        > 0
}

pub fn class(env: &Env, obj: &GObject, deps: &[library::TypeId]) -> Option<Info> {
    info!("Analyzing class {}", obj.name);
    let full_name = obj.name.clone();

    let class_tid = env.library.find_type(0, &full_name)?;

    let type_ = env.type_(class_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let klass: &library::Class = type_.maybe_ref()?;

    let version = obj.version.or(klass.version);
    let deprecated_version = klass.deprecated_version;

    let mut imports = Imports::with_defined(&env.library, &name);
    if obj.generate_display_trait {
        imports.add("std::fmt");
    }

    let is_fundamental = obj.fundamental_type.unwrap_or(klass.is_fundamental);
    let supertypes = supertypes::analyze(env, class_tid, version, &mut imports, is_fundamental);
    let supertypes_properties = supertypes
        .iter()
        .filter_map(|t| match env.type_(t.type_id) {
            Type::Class(c) => Some(&c.properties),
            Type::Interface(i) => Some(&i.properties),
            _ => None,
        })
        .flatten()
        .collect::<Vec<&_>>();

    let final_type = klass.final_type;
    let trait_name = obj
        .trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{name}Ext"));

    let mut signatures = Signatures::with_capacity(klass.functions.len());

    // As we don't generate virtual methods yet, we don't pass imports here
    // it would need to be fixed once work in generating virtual methods is done
    let virtual_methods = functions::analyze(
        env,
        &klass.virtual_methods,
        Some(class_tid),
        true,
        false,
        obj,
        &mut Imports::default(),
        None,
        Some(deps),
    );

    let mut functions = functions::analyze(
        env,
        &klass.functions,
        Some(class_tid),
        !final_type,
        false,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );
    let mut specials = special_functions::extract(&mut functions, type_, obj);
    // `copy` will duplicate an object while `clone` just adds a reference
    special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    // these are all automatically derived on objects and compare by pointer. If
    // such functions exist they will provide additional functionality
    for t in &[
        special_functions::Type::Hash,
        special_functions::Type::Equal,
        special_functions::Type::Compare,
    ] {
        special_functions::unhide(&mut functions, &specials, *t);
        specials.traits_mut().remove(t);
    }
    special_functions::analyze_imports(&specials, &mut imports);

    let signals = signals::analyze(
        env,
        &klass.signals,
        class_tid,
        !final_type,
        is_fundamental,
        obj,
        &mut imports,
    );
    let (properties, notify_signals) = properties::analyze(
        env,
        &klass.properties,
        &supertypes_properties,
        class_tid,
        !final_type,
        is_fundamental,
        obj,
        &mut imports,
        &signatures,
        deps,
    );

    let builder_properties =
        class_builder::analyze(env, &klass.properties, class_tid, obj, &mut imports);

    let child_properties =
        child_properties::analyze(env, obj.child_properties.as_ref(), class_tid, &mut imports);

    let has_methods = functions
        .iter()
        .any(|f| f.kind == library::FunctionKind::Method && f.status.need_generate());
    let has_signals = signals.iter().any(|s| s.trampoline.is_ok())
        || notify_signals.iter().any(|s| s.trampoline.is_ok());
    // There's no point in generating a trait if there are no signals, methods,
    // properties and child properties: it would be empty
    //
    // There's also no point in generating a trait for final types: there are no
    // possible subtypes
    let generate_trait = !final_type
        && !is_fundamental
        && (has_signals || has_methods || !properties.is_empty() || !child_properties.is_empty());

    if is_fundamental {
        imports.add("glib::translate::*");
    }

    if has_builder_properties(&builder_properties) {
        imports.add("glib::prelude::*");
    }

    if generate_trait {
        imports.add("glib::prelude::*");
    }

    let base = InfoBase {
        full_name,
        type_id: class_tid,
        name,
        functions,
        specials,
        imports,
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
        visibility: obj.visibility,
    };

    // patch up trait methods in the symbol table
    if generate_trait {
        let mut symbols = env.symbols.borrow_mut();
        for func in base.methods() {
            if let Some(symbol) = symbols.by_c_name_mut(&func.glib_name) {
                symbol.make_trait_method(&trait_name);
            }
        }
    }

    let has_constructors = !base.constructors().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base,
        c_type: klass.c_type.clone(),
        c_class_type: klass.c_class_type.clone(),
        get_type: klass.glib_get_type.clone(),
        is_interface: false,
        is_fundamental,
        supertypes,
        final_type,
        generate_trait,
        trait_name,
        has_constructors,
        has_functions,
        virtual_methods,
        signals,
        notify_signals,
        properties,
        builder_properties,
        builder_postprocess: obj.builder_postprocess.clone(),
        child_properties,
        signatures,
        ref_fn: klass.ref_fn.clone(),
        unref_fn: klass.unref_fn.clone(),
    };

    Some(info)
}

pub fn interface(env: &Env, obj: &GObject, deps: &[library::TypeId]) -> Option<Info> {
    info!("Analyzing interface {}", obj.name);
    let full_name = obj.name.clone();

    let iface_tid = env.library.find_type(0, &full_name)?;

    let type_ = env.type_(iface_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let iface: &library::Interface = type_.maybe_ref()?;

    let version = obj.version.or(iface.version);
    let deprecated_version = iface.deprecated_version;

    let mut imports = Imports::with_defined(&env.library, &name);
    imports.add("glib::prelude::*");
    if obj.generate_display_trait {
        imports.add("std::fmt");
    }

    let supertypes = supertypes::analyze(env, iface_tid, version, &mut imports, false);
    let supertypes_properties = supertypes
        .iter()
        .filter_map(|t| match env.type_(t.type_id) {
            Type::Class(c) => Some(&c.properties),
            Type::Interface(i) => Some(&i.properties),
            _ => None,
        })
        .flatten()
        .collect::<Vec<&_>>();

    let trait_name = obj
        .trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{name}Ext"));

    let mut signatures = Signatures::with_capacity(iface.functions.len());

    let functions = functions::analyze(
        env,
        &iface.functions,
        Some(iface_tid),
        true,
        false,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );

    let signals = signals::analyze(
        env,
        &iface.signals,
        iface_tid,
        true,
        false,
        obj,
        &mut imports,
    );
    let (properties, notify_signals) = properties::analyze(
        env,
        &iface.properties,
        &supertypes_properties,
        iface_tid,
        true,
        false,
        obj,
        &mut imports,
        &signatures,
        deps,
    );

    let base = InfoBase {
        full_name,
        type_id: iface_tid,
        name,
        functions,
        specials: Default::default(),
        imports,
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
        visibility: obj.visibility,
    };

    let has_functions = !base.functions().is_empty();

    let info = Info {
        base,
        c_type: iface.c_type.clone(),
        c_class_type: iface.c_class_type.clone(),
        get_type: iface.glib_get_type.clone(),
        is_interface: true,
        supertypes,
        final_type: false,
        generate_trait: true,
        trait_name,
        has_functions,
        signals,
        notify_signals,
        properties,
        signatures,
        ..Default::default()
    };

    Some(info)
}
