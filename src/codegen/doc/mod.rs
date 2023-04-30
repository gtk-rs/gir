use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet},
    io::{Result, Write},
};

use log::{error, info};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use stripper_lib::{write_file_name, write_item_doc, Type as SType, TypeStruct};

use self::format::reformat_doc;
use crate::{
    analysis::{self, namespaces::MAIN, object::LocationInObject},
    config::gobjects::GObject,
    env::Env,
    file_saver::save_to_file,
    library::{self, Type as LType, *},
    nameutil,
    traits::*,
    version::Version,
};

mod format;
mod gi_docgen;

// A list of C parameters that are not used directly by the Rust bindings
const IGNORED_C_FN_PARAMS: [&str; 6] = [
    "user_data",
    "user_destroy",
    "destroy_func",
    "dnotify",
    "destroy",
    "user_data_free_func",
];

trait ToStripperType {
    fn to_stripper_type(&self) -> TypeStruct;
}

macro_rules! impl_to_stripper_type {
    ($ty:ident, $enum_var:ident, $useless:expr) => {
        impl ToStripperType for $ty {
            fn to_stripper_type(&self) -> TypeStruct {
                TypeStruct::new(
                    SType::$enum_var,
                    &format!("connect_{}", nameutil::signal_to_snake(&self.name)),
                )
            }
        }
    };
    ($ty:ident, $enum_var:ident) => {
        impl ToStripperType for $ty {
            fn to_stripper_type(&self) -> TypeStruct {
                TypeStruct::new(SType::$enum_var, &self.name)
            }
        }
    };
}

trait FunctionLikeType {
    fn doc(&self) -> &Option<String>;
    fn doc_deprecated(&self) -> &Option<String>;
    fn ret(&self) -> &Parameter;
    fn parameters(&self) -> &[Parameter];
    fn version(&self) -> &Option<Version>;
    fn deprecated_version(&self) -> &Option<Version>;
}

macro_rules! impl_function_like_type {
    ($ty:ident) => {
        impl FunctionLikeType for $ty {
            fn doc(&self) -> &Option<String> {
                &self.doc
            }
            fn doc_deprecated(&self) -> &Option<String> {
                &self.doc_deprecated
            }
            fn ret(&self) -> &Parameter {
                &self.ret
            }
            fn parameters(&self) -> &[Parameter] {
                &self.parameters
            }
            fn version(&self) -> &Option<Version> {
                &self.version
            }
            fn deprecated_version(&self) -> &Option<Version> {
                &self.deprecated_version
            }
        }
    };
}

impl_to_stripper_type!(Enumeration, Enum);
impl_to_stripper_type!(Bitfield, Struct);
impl_to_stripper_type!(Record, Struct);
impl_to_stripper_type!(Class, Struct);
impl_to_stripper_type!(Function, Fn);
impl_to_stripper_type!(Signal, Fn, false);

impl_function_like_type!(Function);
impl_function_like_type!(Signal);

pub fn generate(env: &Env) {
    info!("Generating documentation {:?}", env.config.doc_target_path);
    save_to_file(&env.config.doc_target_path, env.config.make_backup, |w| {
        generate_doc(w, env)
    });
}

#[allow(clippy::type_complexity)]
fn generate_doc(w: &mut dyn Write, env: &Env) -> Result<()> {
    write_file_name(w, None)?;
    let mut generators: Vec<(&str, Box<dyn Fn(&mut dyn Write, &Env) -> Result<()>>)> = Vec::new();

    for info in env.analysis.objects.values() {
        if info.type_id.ns_id == MAIN && !env.is_totally_deprecated(None, info.deprecated_version) {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_object_doc(w, e, info)),
            ));
        }
    }

    for info in env.analysis.records.values() {
        if info.type_id.ns_id == MAIN && !env.is_totally_deprecated(None, info.deprecated_version) {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_record_doc(w, e, info)),
            ));
        }
    }

    for (tid, type_) in env.library.namespace_types(MAIN) {
        if let LType::Enumeration(enum_) = type_ {
            if !env
                .config
                .objects
                .get(&tid.full_name(&env.library))
                .map_or(true, |obj| obj.status.ignored())
                && !env.is_totally_deprecated(None, enum_.deprecated_version)
            {
                generators.push((
                    enum_.name.as_str(),
                    Box::new(move |w, e| create_enum_doc(w, e, enum_, tid)),
                ));
            }
        } else if let LType::Bitfield(bitfield) = type_ {
            if !env
                .config
                .objects
                .get(&tid.full_name(&env.library))
                .map_or(true, |obj| obj.status.ignored())
                && !env.is_totally_deprecated(None, bitfield.deprecated_version)
            {
                generators.push((
                    bitfield.name.as_str(),
                    Box::new(move |w, e| create_bitfield_doc(w, e, bitfield, tid)),
                ));
            }
        }
    }

    let ns = env.library.namespace(library::MAIN_NAMESPACE);

    if let Some(ref global_functions) = env.analysis.global_functions {
        let functions = ns
            .functions
            .iter()
            .filter(|f| f.kind == library::FunctionKind::Global);

        for function in functions {
            if let Some(ref c_identifier) = function.c_identifier {
                let f_info = global_functions
                    .functions
                    .iter()
                    .find(move |f| &f.glib_name == c_identifier);
                let fn_new_name = f_info.and_then(|analysed_f| analysed_f.new_name.clone());
                let doc_trait_name = f_info.and_then(|f| f.doc_trait_name.as_ref());
                let doc_struct_name = f_info.and_then(|f| f.doc_struct_name.as_ref());
                assert!(
                    !(doc_trait_name.is_some() && doc_struct_name.is_some()),
                    "Can't use both doc_trait_name and doc_struct_name on the same function"
                );

                let parent = if doc_trait_name.is_some() {
                    doc_trait_name.map(|p| Box::new(TypeStruct::new(SType::Trait, p)))
                } else if doc_struct_name.is_some() {
                    doc_struct_name.map(|p| Box::new(TypeStruct::new(SType::Impl, p)))
                } else {
                    None
                };

                let doc_ignored_parameters = f_info
                    .map(|analyzed_f| analyzed_f.doc_ignore_parameters.clone())
                    .unwrap_or_default();

                let should_be_documented =
                    f_info.map_or(false, |f| f.should_docs_be_generated(env));
                if !should_be_documented {
                    continue;
                }

                create_fn_doc(
                    w,
                    env,
                    function,
                    parent,
                    fn_new_name,
                    &doc_ignored_parameters,
                    None,
                    f_info.map_or(true, |f| f.generate_doc),
                )?;
            }
        }
    }

    for constant in &ns.constants {
        // strings are mapped to a static
        let ty = if constant.c_type == "gchar*" {
            SType::Static
        } else {
            SType::Const
        };
        let ty_id = TypeStruct::new(ty, &constant.name);
        let generate_doc = env
            .config
            .objects
            .get(&constant.typ.full_name(&env.library))
            .map_or(true, |c| c.generate_doc);
        if generate_doc {
            write_item_doc(w, &ty_id, |w| {
                if let Some(ref doc) = constant.doc {
                    writeln!(w, "{}", reformat_doc(doc, env, Some((&constant.typ, None))))?;
                }
                Ok(())
            })?;
        }
    }

    generators.sort_by_key(|&(name, _)| name);
    for (_, f) in generators {
        f(w, env)?;
    }

    Ok(())
}

fn create_object_doc(w: &mut dyn Write, env: &Env, info: &analysis::object::Info) -> Result<()> {
    let ty = TypeStruct::new(SType::Struct, &info.name);
    let ty_ext = TypeStruct::new(SType::Trait, &info.trait_name);
    let has_trait = info.generate_trait;
    let doc;
    let doc_deprecated;
    let functions: &[Function];
    let virtual_methods: &[Function];
    let signals: &[Signal];
    let properties: &[Property];
    let is_abstract;
    let has_builder;

    let obj = env
        .config
        .objects
        .get(&info.full_name)
        .expect("Object not found");

    match env.library.type_(info.type_id) {
        Type::Class(cl) => {
            doc = cl.doc.as_ref();
            doc_deprecated = cl.doc_deprecated.as_ref();
            functions = &cl.functions;
            virtual_methods = &cl.virtual_methods;
            signals = &cl.signals;
            properties = &cl.properties;
            is_abstract = env.library.type_(info.type_id).is_abstract();
            has_builder = obj.generate_builder;
        }
        Type::Interface(iface) => {
            doc = iface.doc.as_ref();
            doc_deprecated = iface.doc_deprecated.as_ref();
            functions = &iface.functions;
            virtual_methods = &iface.virtual_methods;
            signals = &iface.signals;
            properties = &iface.properties;
            is_abstract = false;
            has_builder = false;
        }
        _ => unreachable!(),
    }

    let manual_traits = get_type_manual_traits_for_implements(env, info);

    write_item_doc(w, &ty, |w| {
        if let Some(doc) = doc_deprecated {
            writeln!(
                w,
                "{}",
                reformat_doc(
                    doc,
                    env,
                    Some((&info.type_id, Some(LocationInObject::Impl)))
                )
            )?;
        }
        if let (Some(doc), true) = (doc, obj.generate_doc) {
            writeln!(
                w,
                "{}",
                reformat_doc(
                    doc,
                    env,
                    Some((&info.type_id, Some(LocationInObject::Impl)))
                )
            )?;
        } else {
            writeln!(w)?;
        }
        if is_abstract {
            writeln!(
                w,
                "\nThis is an Abstract Base Class, you cannot instantiate it."
            )?;
        }

        if !properties.is_empty() {
            writeln!(w, "\n## Properties")?;
            document_type_properties(env, w, info, properties, None)?;

            for parent_info in &info.supertypes {
                match env.library.type_(parent_info.type_id) {
                    Type::Class(cl) => {
                        if !cl.properties.is_empty() {
                            document_type_properties(env, w, info, &cl.properties, Some(&cl.name))?;
                        }
                    }
                    Type::Interface(iface) => {
                        if !iface.properties.is_empty() {
                            document_type_properties(
                                env,
                                w,
                                info,
                                &iface.properties,
                                Some(&iface.name),
                            )?;
                        }
                    }
                    _ => (),
                }
            }
        }
        if !signals.is_empty() {
            writeln!(w, "\n## Signals")?;
            document_type_signals(env, w, info, signals, None)?;

            for parent_info in &info.supertypes {
                match env.library.type_(parent_info.type_id) {
                    Type::Class(cl) => {
                        if !cl.signals.is_empty() {
                            document_type_signals(env, w, info, &cl.signals, Some(&cl.name))?;
                        }
                    }
                    Type::Interface(iface) => {
                        if !iface.signals.is_empty() {
                            document_type_signals(env, w, info, &iface.signals, Some(&iface.name))?;
                        }
                    }
                    _ => (),
                }
            }
        }

        let impl_self = if has_trait { Some(info.type_id) } else { None };
        let mut implements = impl_self
            .iter()
            .chain(env.class_hierarchy.supertypes(info.type_id))
            .filter(|&tid| {
                !env.type_status(&tid.full_name(&env.library)).ignored()
                    && !env.type_(*tid).is_final_type()
                    && !env.type_(*tid).is_fundamental()
            })
            .map(|&tid| get_type_trait_for_implements(env, tid))
            .collect::<Vec<_>>();
        implements.extend(manual_traits);

        if !implements.is_empty() {
            writeln!(w, "\n# Implements\n")?;
            writeln!(w, "{}", &implements.join(", "))?;
        }
        Ok(())
    })?;

    if has_builder {
        let builder_ty = TypeStruct::new(SType::Impl, &format!("{}Builder", info.name));
        let mut builder_properties: Vec<_> = properties.iter().collect();
        for parent_info in &info.supertypes {
            match env.library.type_(parent_info.type_id) {
                Type::Class(cl) => {
                    builder_properties.extend(cl.properties.iter().filter(|p| p.writable));
                }
                Type::Interface(iface) => {
                    builder_properties.extend(iface.properties.iter().filter(|p| p.writable));
                }
                _ => (),
            }
        }
        for property in &builder_properties {
            let ty = TypeStruct {
                ty: SType::Fn,
                name: nameutil::signal_to_snake(&property.name),
                parent: Some(Box::new(builder_ty.clone())),
                args: vec![],
            };
            write_item_doc(w, &ty, |w| {
                if let Some(ref doc) = property.doc {
                    writeln!(
                        w,
                        "{}",
                        reformat_doc(
                            &fix_param_names(doc, &None),
                            env,
                            Some((&info.type_id, Some(LocationInObject::Builder)))
                        )
                    )?;
                }
                if let Some(ref doc) = property.doc_deprecated {
                    writeln!(
                        w,
                        "{}",
                        reformat_doc(
                            &fix_param_names(doc, &None),
                            env,
                            Some((&info.type_id, Some(LocationInObject::Builder)))
                        )
                    )?;
                }
                Ok(())
            })?;
        }
    }

    if has_trait {
        write_item_doc(w, &ty_ext, |w| {
            writeln!(w, "Trait containing all [`struct@{}`] methods.", ty.name)?;

            let mut implementors = std::iter::once(info.type_id)
                .chain(env.class_hierarchy.subtypes(info.type_id))
                .filter(|&tid| !env.type_status(&tid.full_name(&env.library)).ignored())
                .map(|tid| {
                    format!(
                        "[`{0}`][struct@crate::{0}]",
                        env.library.type_(tid).get_name()
                    )
                })
                .collect::<Vec<_>>();
            implementors.sort();

            writeln!(w, "\n# Implementors\n")?;
            writeln!(w, "{}", implementors.join(", "))?;
            Ok(())
        })?;
    }

    let ty = TypeStruct {
        ty: SType::Impl,
        ..ty
    };

    for function in functions {
        let configured_functions = obj.functions.matched(&function.name);
        let is_manual = configured_functions.iter().any(|f| f.status.manual());
        let (ty, object_location) = if (has_trait || is_manual)
            && function.parameters.iter().any(|p| p.instance_parameter)
            && !info.final_type
        {
            if let Some(struct_name) = configured_functions
                .iter()
                .find_map(|f| f.doc_struct_name.as_ref())
            {
                (
                    TypeStruct::new(SType::Impl, struct_name),
                    Some(LocationInObject::Impl),
                )
            }
            // We use "original_name" here to be sure to get the correct object since the "name"
            // field could have been renamed.
            else if let Some(trait_name) = configured_functions
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                (
                    TypeStruct::new(SType::Trait, trait_name),
                    // Because we cannot sensibly deduce where the docs end up,
                    // assume they're outside the docs so that no `Self::` links
                    // are generated.  It is currently quite uncommon to specify
                    // the `{}Manual` trait, which would be ObjectLocation::ExtManual.
                    None,
                )
            } else if is_manual {
                (
                    TypeStruct::new(SType::Trait, &format!("{}ExtManual", info.name)),
                    Some(LocationInObject::ExtManual),
                )
            } else {
                (ty_ext.clone(), Some(LocationInObject::Ext))
            }
        } else {
            (ty.clone(), Some(LocationInObject::Impl))
        };
        if let Some(c_identifier) = &function.c_identifier {
            let f_info = info.functions.iter().find(|f| &f.glib_name == c_identifier);
            let should_be_documented = f_info.map_or(false, |f| f.should_docs_be_generated(env));

            if !should_be_documented {
                continue;
            }

            // Retrieve the new_name computed during analysis, if any
            let fn_new_name = f_info.and_then(|analysed_f| analysed_f.new_name.clone());
            let doc_ignored_parameters = f_info
                .map(|analyzed_f| analyzed_f.doc_ignore_parameters.clone())
                .unwrap_or_default();
            create_fn_doc(
                w,
                env,
                function,
                Some(Box::new(ty)),
                fn_new_name,
                &doc_ignored_parameters,
                Some((&info.type_id, object_location)),
                f_info.map_or(true, |f| f.generate_doc),
            )?;
        }
    }
    for signal in signals {
        let configured_signals = obj.signals.matched(&signal.name);
        let (ty, object_location) = if has_trait {
            if let Some(trait_name) = configured_signals
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                (TypeStruct::new(SType::Trait, trait_name), None)
            } else {
                (ty_ext.clone(), Some(LocationInObject::Ext))
            }
        } else {
            (ty.clone(), Some(LocationInObject::Impl))
        };
        create_fn_doc(
            w,
            env,
            signal,
            Some(Box::new(ty)),
            None,
            &HashSet::new(),
            Some((&info.type_id, object_location)),
            configured_signals.iter().all(|s| s.generate_doc),
        )?;
    }

    for function in virtual_methods {
        let configured_virtual_methods = obj.virtual_methods.matched(&function.name);
        let (ty, object_location) = if let Some(trait_name) = configured_virtual_methods
            .iter()
            .find_map(|f| f.doc_trait_name.as_ref())
        {
            (
                TypeStruct::new(SType::Trait, trait_name),
                // Because we cannot sensibly deduce where the docs end up,
                // assume they're outside the docs so that no `Self::` links
                // are generated.  It is currently quite uncommon to specify
                // the `{}Manual` trait, which would be ObjectLocation::ExtManual.
                None,
            )
        } else {
            (
                TypeStruct::new(SType::Trait, &format!("{}Impl", info.name)),
                Some(LocationInObject::VirtualExt),
            )
        };

        if let Some(c_identifier) = &function.c_identifier {
            let f_info: Option<&analysis::functions::Info> = info
                .virtual_methods
                .iter()
                .find(|f| &f.glib_name == c_identifier);
            let should_be_documented = f_info.map_or(false, |f| f.should_docs_be_generated(env));
            if !should_be_documented {
                continue;
            }

            // Retrieve the new_name computed during analysis, if any
            let fn_new_name = f_info.and_then(|analysed_f| analysed_f.new_name.clone());
            let doc_ignored_parameters = f_info
                .map(|analyzed_f| analyzed_f.doc_ignore_parameters.clone())
                .unwrap_or_default();
            create_fn_doc(
                w,
                env,
                function,
                Some(Box::new(ty)),
                fn_new_name,
                &doc_ignored_parameters,
                Some((&info.type_id, object_location)),
                f_info.map_or(true, |f| f.generate_doc),
            )?;
        }
    }

    for property in properties {
        let getter_name = info
            .properties
            .iter()
            .filter(|p| p.is_get)
            .find(|p| p.name == property.name)
            .map(|p| p.func_name.clone());

        let setter_name = info
            .properties
            .iter()
            .filter(|p| !p.is_get)
            .find(|p| p.name == property.name)
            .map(|p| p.func_name.clone());
        let (ty, object_location) = if has_trait {
            let configured_properties = obj.properties.matched(&property.name);
            if let Some(trait_name) = configured_properties
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                (TypeStruct::new(SType::Trait, trait_name), None)
            } else {
                (ty_ext.clone(), Some(LocationInObject::Ext))
            }
        } else {
            (ty.clone(), Some(LocationInObject::Impl))
        };
        create_property_doc(
            w,
            env,
            property,
            Some(Box::new(ty)),
            (&info.type_id, object_location),
            getter_name,
            setter_name,
            info,
        )?;
    }
    Ok(())
}

fn create_record_doc(w: &mut dyn Write, env: &Env, info: &analysis::record::Info) -> Result<()> {
    let record: &Record = env.library.type_(info.type_id).to_ref_as();
    let ty = record.to_stripper_type();
    let object = env.config.objects.get(&info.full_name);
    let trait_name = object
        .and_then(|o| o.trait_name.clone())
        .unwrap_or_else(|| format!("{}Ext", info.name));
    let generate_doc = object.map_or(true, |r| r.generate_doc);
    if generate_doc {
        write_item_doc(w, &ty, |w| {
            if let Some(ref doc) = record.doc {
                writeln!(w, "{}", reformat_doc(doc, env, Some((&info.type_id, None))))?;
            }
            if let Some(ver) = info.deprecated_version {
                writeln!(w, "\n# Deprecated since {ver}\n")?;
            } else if record.doc_deprecated.is_some() {
                writeln!(w, "\n# Deprecated\n")?;
            }
            if let Some(ref doc) = record.doc_deprecated {
                writeln!(w, "{}", reformat_doc(doc, env, Some((&info.type_id, None))))?;
            }
            Ok(())
        })?;
    }

    for function in &record.functions {
        let function_ty = if function.kind == FunctionKind::ClassMethod {
            TypeStruct::new(SType::Trait, &trait_name)
        } else {
            TypeStruct {
                ty: SType::Impl,
                parent: ty.parent.clone(),
                name: ty.name.clone(),
                args: ty.args.clone(),
            }
        };
        if let Some(c_identifier) = &function.c_identifier {
            let f_info = info.functions.iter().find(|f| &f.glib_name == c_identifier);
            let should_be_documented = f_info.map_or(false, |f| f.should_docs_be_generated(env));
            if !should_be_documented {
                continue;
            }
            let fn_new_name = f_info.and_then(|analysed_f| analysed_f.new_name.clone());

            create_fn_doc(
                w,
                env,
                function,
                Some(Box::new(function_ty)),
                fn_new_name,
                &HashSet::new(),
                Some((&info.type_id, None)),
                f_info.map_or(true, |f| f.generate_doc),
            )?;
        }
    }
    Ok(())
}

fn create_enum_doc(w: &mut dyn Write, env: &Env, enum_: &Enumeration, tid: TypeId) -> Result<()> {
    let ty = enum_.to_stripper_type();
    let config = env.config.objects.get(&tid.full_name(&env.library));

    if config.map_or(true, |c| c.generate_doc) {
        write_item_doc(w, &ty, |w| {
            if let Some(ref doc) = enum_.doc {
                writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
            }
            if let Some(ver) = enum_.deprecated_version {
                writeln!(w, "\n# Deprecated since {ver}\n")?;
            } else if enum_.doc_deprecated.is_some() {
                writeln!(w, "\n# Deprecated\n")?;
            }
            if let Some(ref doc) = enum_.doc_deprecated {
                writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
            }
            Ok(())
        })?;
    }

    for member in &enum_.members {
        let generate_doc = config
            .and_then(|m| {
                m.members
                    .matched(&member.name)
                    .first()
                    .map(|m| m.generate_doc)
            })
            .unwrap_or(true);

        if generate_doc && member.doc.is_some() {
            let sub_ty = TypeStruct {
                name: nameutil::enum_member_name(&member.name),
                parent: Some(Box::new(ty.clone())),
                ty: SType::Variant,
                args: Vec::new(),
            };
            write_item_doc(w, &sub_ty, |w| {
                if let Some(ref doc) = member.doc {
                    writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
                }
                if let Some(ref doc) = member.doc_deprecated {
                    writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
                }
                Ok(())
            })?;
        }
    }

    Ok(())
}

fn create_bitfield_doc(
    w: &mut dyn Write,
    env: &Env,
    bitfield: &Bitfield,
    tid: TypeId,
) -> Result<()> {
    let ty = bitfield.to_stripper_type();
    let config = env.config.objects.get(&tid.full_name(&env.library));

    write_item_doc(w, &ty, |w| {
        if config.map_or(true, |c| c.generate_doc) {
            if let Some(ref doc) = bitfield.doc {
                writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
            }
        }
        if let Some(ver) = bitfield.deprecated_version {
            writeln!(w, "\n# Deprecated since {ver}\n")?;
        } else if bitfield.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = bitfield.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
        }
        Ok(())
    })?;

    for member in &bitfield.members {
        let generate_doc = config
            .and_then(|m| {
                m.members
                    .matched(&member.name)
                    .first()
                    .map(|m| m.generate_doc)
            })
            .unwrap_or(true);

        if generate_doc && member.doc.is_some() {
            let sub_ty = TypeStruct {
                name: nameutil::bitfield_member_name(&member.name),
                parent: Some(Box::new(ty.clone())),
                ty: SType::Const,
                args: Vec::new(),
            };
            write_item_doc(w, &sub_ty, |w| {
                if let Some(ref doc) = member.doc {
                    writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
                }
                if let Some(ref doc) = member.doc_deprecated {
                    writeln!(w, "{}", reformat_doc(doc, env, Some((&tid, None))))?;
                }
                Ok(())
            })?;
        }
    }

    Ok(())
}

static PARAM_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"@(\w+)\b").unwrap());

fn fix_param_names<'a>(doc: &'a str, self_name: &Option<String>) -> Cow<'a, str> {
    PARAM_NAME.replace_all(doc, |caps: &Captures<'_>| {
        if let Some(self_name) = self_name {
            if &caps[1] == self_name {
                return "@self".into();
            }
        }
        format!("@{}", nameutil::mangle_keywords(&caps[1]))
    })
}

fn create_fn_doc<T>(
    w: &mut dyn Write,
    env: &Env,
    fn_: &T,
    parent: Option<Box<TypeStruct>>,
    name_override: Option<String>,
    doc_ignored_parameters: &HashSet<String>,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
    generate_doc: bool,
) -> Result<()>
where
    T: FunctionLikeType + ToStripperType,
{
    if !generate_doc {
        return Ok(());
    }
    if env.is_totally_deprecated(None, *fn_.deprecated_version()) {
        return Ok(());
    }
    if fn_.doc().is_none()
        && fn_.doc_deprecated().is_none()
        && fn_.ret().doc.is_none()
        && fn_.parameters().iter().all(|p| p.doc.is_none())
    {
        return Ok(());
    }

    let mut st = fn_.to_stripper_type();
    if let Some(name_override) = name_override {
        st.name = name_override;
    }
    let ty = TypeStruct { parent, ..st };
    let self_name: Option<String> = fn_
        .parameters()
        .iter()
        .find(|p| p.instance_parameter)
        .map(|p| p.name.clone());

    write_item_doc(w, &ty, |w| {
        if let Some(doc) = fn_.doc() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), env, in_type)
            )?;
        }
        if let Some(ver) = fn_.deprecated_version() {
            writeln!(w, "\n# Deprecated since {ver}\n")?;
        } else if fn_.doc_deprecated().is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(doc) = fn_.doc_deprecated() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), env, in_type)
            )?;
        }

        // A list of parameter positions to filter out
        let mut indices_to_ignore: BTreeSet<_> = fn_
            .parameters()
            .iter()
            .filter_map(|param| param.array_length)
            .collect();
        if let Some(indice) = fn_.ret().array_length {
            indices_to_ignore.insert(indice);
        }

        // The original list of parameters without the ones that specify an array length
        let no_array_length_params: Vec<_> = fn_
            .parameters()
            .iter()
            .enumerate()
            .filter_map(|(indice, param)| {
                (!indices_to_ignore.contains(&(indice as u32))).then_some(param)
            })
            .filter(|param| !param.instance_parameter)
            .collect();

        let in_parameters = no_array_length_params.iter().filter(|param| {
            let ignore = IGNORED_C_FN_PARAMS.contains(&param.name.as_str())
                || doc_ignored_parameters.contains(&param.name)
                || param.direction == ParameterDirection::Out
                // special case error pointer as it's transformed to a Result
                || (param.name == "error" && param.c_type == "GError**")
                // special case `data` with explicit `gpointer` type as it could be something else (unlike `user_data`)
                || (param.name == "data" && param.c_type == "gpointer");
            !ignore
        });

        for parameter in in_parameters {
            if parameter.name.is_empty() {
                continue;
            }
            if let Some(ref doc) = parameter.doc {
                writeln!(
                    w,
                    "## `{}`",
                    nameutil::mangle_keywords(parameter.name.as_str())
                )?;
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &self_name), env, in_type)
                )?;
            }
        }

        let out_parameters: Vec<_> = no_array_length_params
            .iter()
            .filter(|param| {
                param.direction == ParameterDirection::Out
                    && !doc_ignored_parameters.contains(&param.name)
                    && !(param.name == "error" && param.c_type == "GError**")
            })
            .collect();

        if fn_.ret().doc.is_some() || !out_parameters.is_empty() {
            writeln!(w, "\n# Returns\n")?;
        }
        // document function's return
        if let Some(ref doc) = fn_.ret().doc {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), env, in_type)
            )?;
        }
        // document OUT parameters as part of the function's Return
        for parameter in out_parameters {
            if let Some(ref doc) = parameter.doc {
                writeln!(
                    w,
                    "\n## `{}`",
                    nameutil::mangle_keywords(parameter.name.as_str())
                )?;
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &self_name), env, in_type),
                )?;
            }
        }
        Ok(())
    })
}

fn create_property_doc(
    w: &mut dyn Write,
    env: &Env,
    property: &Property,
    parent: Option<Box<TypeStruct>>,
    in_type: (&TypeId, Option<LocationInObject>),
    getter_name: Option<String>,
    setter_name: Option<String>,
    obj_info: &analysis::object::Info,
) -> Result<()> {
    if env.is_totally_deprecated(Some(in_type.0.ns_id), property.deprecated_version) {
        return Ok(());
    }
    let generate_doc = env
        .config
        .objects
        .get(&obj_info.type_id.full_name(&env.library))
        .map_or(true, |r| r.generate_doc);
    if !generate_doc {
        return Ok(());
    }
    if property.doc.is_none()
        && property.doc_deprecated.is_none()
        && (property.readable || property.writable)
    {
        return Ok(());
    }
    let name_for_func = nameutil::signal_to_snake(&property.name);
    let getter_name = getter_name.unwrap_or_else(|| name_for_func.clone());
    let has_getter_method = obj_info.functions.iter().any(|f| {
        f.func_name == getter_name || f.new_name.as_ref().map_or(false, |n| n == &getter_name)
    });

    let setter_name = setter_name.unwrap_or_else(|| format!("set_{}", &name_for_func));
    let has_setter_method = obj_info.functions.iter().any(|f| {
        f.func_name == setter_name || f.new_name.as_ref().map_or(false, |n| n == &setter_name)
    });

    let mut v = Vec::with_capacity(2);

    if property.readable && !has_getter_method {
        v.push(TypeStruct {
            parent: parent.clone(),
            ..TypeStruct::new(SType::Fn, &getter_name)
        });
    }
    if property.writable && !property.construct_only && !has_setter_method {
        v.push(TypeStruct {
            parent,
            ..TypeStruct::new(SType::Fn, &setter_name)
        });
    }

    for item in &v {
        write_item_doc(w, item, |w| {
            if let Some(ref doc) = property.doc {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), env, Some(in_type))
                )?;
            }
            if let Some(ver) = property.deprecated_version {
                writeln!(w, "\n# Deprecated since {ver}\n")?;
            } else if property.doc_deprecated.is_some() {
                writeln!(w, "\n# Deprecated\n")?;
            }
            if let Some(ref doc) = property.doc_deprecated {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), env, Some(in_type))
                )?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn get_type_trait_for_implements(env: &Env, tid: TypeId) -> String {
    let trait_name = if let Some(&GObject {
        trait_name: Some(ref trait_name),
        ..
    }) = env.config.objects.get(&tid.full_name(&env.library))
    {
        trait_name.clone()
    } else {
        format!("{}Ext", env.library.type_(tid).get_name())
    };
    if tid.ns_id == MAIN_NAMESPACE {
        format!("[`{trait_name}`][trait@crate::prelude::{trait_name}]")
    } else if let Some(symbol) = env.symbols.borrow().by_tid(tid) {
        let mut symbol = symbol.clone();
        symbol.make_trait(&trait_name);
        format!("[`trait@{}`]", &symbol.full_rust_name())
    } else {
        error!("Type {} doesn't have crate", tid.full_name(&env.library));
        format!("`{trait_name}`")
    }
}

pub fn get_type_manual_traits_for_implements(
    env: &Env,
    info: &analysis::object::Info,
) -> Vec<String> {
    let mut manual_trait_iters = Vec::new();
    for type_id in [info.type_id]
        .iter()
        .chain(info.supertypes.iter().map(|stid| &stid.type_id))
    {
        let full_name = type_id.full_name(&env.library);
        if let Some(obj) = &env.config.objects.get(&full_name) {
            if !obj.manual_traits.is_empty() {
                manual_trait_iters.push(obj.manual_traits.iter());
            }
        }
    }

    manual_trait_iters
        .into_iter()
        .flatten()
        .map(|name| format!("[`{name}`][trait@crate::prelude::{name}]"))
        .collect()
}

pub fn document_type_properties(
    env: &Env,
    w: &mut dyn Write,
    info: &analysis::object::Info,
    properties: &[Property],
    subtype: Option<&str>,
) -> Result<()> {
    if let Some(subtype_name) = subtype {
        writeln!(w, "<details><summary><h4>{subtype_name}</h4></summary>")?;
    }
    for property in properties {
        let mut details = Vec::new();
        if property.readable {
            details.push("Readable");
        }
        if property.writable {
            details.push("Writeable");
        }
        if property.construct {
            details.push("Construct");
        }
        if property.construct_only {
            details.push("Construct Only");
        }
        if let Some(doc) = &property.doc {
            writeln!(
                w,
                "\n\n#### `{}`\n {}\n\n{}",
                property.name,
                reformat_doc(
                    &fix_param_names(doc, &None),
                    env,
                    Some((&info.type_id, None))
                ),
                details.join(" | "),
            )?;
        } else {
            writeln!(w, "\n\n#### `{}`\n {}", property.name, details.join(" | "),)?;
        }
    }
    if subtype.is_some() {
        writeln!(w, "</details>")?;
    }
    Ok(())
}

pub fn document_type_signals(
    env: &Env,
    w: &mut dyn Write,
    info: &analysis::object::Info,
    signals: &[Signal],
    subtype: Option<&str>,
) -> Result<()> {
    if let Some(subtype_name) = subtype {
        writeln!(w, "<details><summary><h4>{subtype_name}</h4></summary>")?;
    }
    for signal in signals {
        let mut details = Vec::new();
        if signal.is_action {
            details.push("Action");
        }
        if signal.is_detailed {
            details.push("Detailed");
        }
        if let Some(doc) = &signal.doc {
            writeln!(
                w,
                "\n\n#### `{}`\n {}\n\n{}",
                signal.name,
                reformat_doc(
                    &fix_param_names(doc, &None),
                    env,
                    Some((&info.type_id, None))
                ),
                details.join(" | "),
            )?;
        } else {
            writeln!(w, "\n\n#### `{}`\n {}", signal.name, details.join(" | "),)?;
        }
    }
    if subtype.is_some() {
        writeln!(w, "</details>")?;
    }
    Ok(())
}
