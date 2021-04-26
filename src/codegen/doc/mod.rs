use self::format::reformat_doc;
use crate::{
    analysis::{self, namespaces::MAIN},
    case::CaseExt,
    config::gobjects::GObject,
    env::Env,
    file_saver::save_to_file,
    library::{Type as LType, *},
    nameutil,
    traits::*,
    version::Version,
};
use log::{error, info};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::{
    borrow::Cow,
    io::{Result, Write},
};
use stripper_lib::{write_file_name, write_item_doc, Type as SType, TypeStruct};

mod format;

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
        if info.type_id.ns_id == MAIN && !env.is_totally_deprecated(info.deprecated_version) {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_object_doc(w, e, info)),
            ));
        }
    }

    for info in env.analysis.records.values() {
        if info.type_id.ns_id == MAIN && !env.is_totally_deprecated(info.deprecated_version) {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_record_doc(w, e, info)),
            ));
        }
    }

    for (tid, type_) in env.library.namespace_types(MAIN) {
        if let LType::Enumeration(ref enum_) = *type_ {
            if !env
                .config
                .objects
                .get(&tid.full_name(&env.library))
                .map_or(true, |obj| obj.status.ignored())
                && !env.is_totally_deprecated(enum_.deprecated_version)
            {
                generators.push((
                    &enum_.name[..],
                    Box::new(move |w, e| create_enum_doc(w, e, enum_)),
                ));
            }
        } else if let LType::Bitfield(ref bitfield) = *type_ {
            if !env
                .config
                .objects
                .get(&tid.full_name(&env.library))
                .map_or(true, |obj| obj.status.ignored())
                && !env.is_totally_deprecated(bitfield.deprecated_version)
            {
                generators.push((
                    &bitfield.name[..],
                    Box::new(move |w, e| create_bitfield_doc(w, e, bitfield)),
                ));
            }
        }
    }

    generators.sort_by_key(|&(name, _)| name);
    for (_, f) in generators {
        f(w, env)?;
    }

    Ok(())
}

fn create_object_doc(w: &mut dyn Write, env: &Env, info: &analysis::object::Info) -> Result<()> {
    let symbols = env.symbols.borrow();
    let ty = TypeStruct::new(SType::Struct, &info.name);
    let ty_ext = TypeStruct::new(SType::Trait, &info.trait_name);
    let has_trait = info.generate_trait;
    let doc;
    let doc_deprecated;
    let functions: &[Function];
    let signals: &[Signal];
    let properties: &[Property];
    let is_abstract;

    let obj = env
        .config
        .objects
        .get(&info.full_name)
        .expect("Object not found");

    match *env.library.type_(info.type_id) {
        Type::Class(ref cl) => {
            doc = cl.doc.as_ref();
            doc_deprecated = cl.doc_deprecated.as_ref();
            functions = &cl.functions;
            signals = &cl.signals;
            properties = &cl.properties;
            is_abstract = env.library.type_(info.type_id).is_abstract();
        }
        Type::Interface(ref iface) => {
            doc = iface.doc.as_ref();
            doc_deprecated = iface.doc_deprecated.as_ref();
            functions = &iface.functions;
            signals = &iface.signals;
            properties = &iface.properties;
            is_abstract = false;
        }
        _ => unreachable!(),
    }

    let manual_traits = get_type_manual_traits_for_implements(env, info);

    write_item_doc(w, &ty, |w| {
        if let Some(ver) = info.deprecated_version {
            write!(w, "`[Deprecated since {}]` ", ver)?;
        }
        if let Some(doc) = doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &info.name))?;
        } else if doc_deprecated.is_some() {
            write!(w, "`[Deprecated]` ")?;
        }
        if let Some(doc) = doc {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &info.name))?;
        } else {
            writeln!(w)?;
        }
        if is_abstract {
            writeln!(
                w,
                "\nThis is an Abstract Base Class, you cannot instantiate it."
            )?;
        }
        if let Some(version) = info.version {
            writeln!(w, "\nFeature: `{}`", version.to_feature())?;
        }

        let impl_self = if has_trait { Some(info.type_id) } else { None };
        let mut implements = impl_self
            .iter()
            .chain(env.class_hierarchy.supertypes(info.type_id))
            .filter(|&tid| !env.type_status(&tid.full_name(&env.library)).ignored())
            .map(|&tid| get_type_trait_for_implements(env, tid))
            .collect::<Vec<_>>();
        implements.extend(manual_traits);

        if !implements.is_empty() {
            writeln!(w, "\n# Implements\n")?;
            writeln!(w, "{}", &implements.join(", "))?;
        }
        Ok(())
    })?;

    if has_trait {
        write_item_doc(w, &ty_ext, |w| {
            if let Some(ver) = info.deprecated_version {
                write!(w, "`[Deprecated since {}]` ", ver)?;
            } else if doc_deprecated.is_some() {
                write!(w, "`[Deprecated]` ")?;
            }
            writeln!(w, "Trait containing all `{}` methods.", ty.name)?;

            if let Some(version) = info.version {
                writeln!(w, "\nFeature: `{}`", version.to_feature())?;
            }

            let mut implementors = Some(info.type_id)
                .into_iter()
                .chain(env.class_hierarchy.subtypes(info.type_id))
                .filter(|&tid| !env.type_status(&tid.full_name(&env.library)).ignored())
                .map(|tid| {
                    format!(
                        "[`{n}`](struct@crate::{n})",
                        n = env.library.type_(tid).get_name()
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
        let ty = if has_trait && function.parameters.iter().any(|p| p.instance_parameter) {
            // We use "original_name" here to be sure to get the correct object since the "name"
            // field could have been renamed.
            if let Some(trait_name) = configured_functions
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                TypeStruct::new(SType::Trait, trait_name)
            } else {
                ty_ext.clone()
            }
        } else {
            ty.clone()
        };
        if let Some(c_identifier) = &function.c_identifier {
            // Retrieve the new_name computed during analysis, if any
            let fn_new_name = info
                .functions
                .iter()
                .find(|f| &f.glib_name == c_identifier)
                .and_then(|analysed_f| analysed_f.new_name.clone());
            create_fn_doc(w, env, function, Some(Box::new(ty)), fn_new_name)?;
        }
    }
    for signal in signals {
        let ty = if has_trait {
            let configured_signals = obj.signals.matched(&signal.name);
            if let Some(trait_name) = configured_signals
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                TypeStruct::new(SType::Trait, trait_name)
            } else {
                ty_ext.clone()
            }
        } else {
            ty.clone()
        };
        create_fn_doc(w, env, signal, Some(Box::new(ty)), None)?;
    }
    for property in properties {
        let ty = if has_trait {
            let configured_properties = obj.properties.matched(&property.name);
            if let Some(trait_name) = configured_properties
                .iter()
                .find_map(|f| f.doc_trait_name.as_ref())
            {
                TypeStruct::new(SType::Trait, trait_name)
            } else {
                ty_ext.clone()
            }
        } else {
            ty.clone()
        };
        create_property_doc(w, env, property, Some(Box::new(ty)))?;
    }
    Ok(())
}

fn create_record_doc(w: &mut dyn Write, env: &Env, info: &analysis::record::Info) -> Result<()> {
    let record: &Record = env.library.type_(info.type_id).to_ref_as();
    let ty = record.to_stripper_type();
    let symbols = env.symbols.borrow();

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = record.doc {
            if let Some(ver) = info.deprecated_version {
                write!(w, "`[Deprecated since {}]` ", ver)?;
            }
            writeln!(w, "{}", reformat_doc(doc, &symbols, &info.name))?;
        }
        if let Some(ver) = info.deprecated_version {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if record.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = record.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &info.name))?;
        }
        if let Some(version) = info.version {
            writeln!(w, "\nFeature: `{}`", version.to_feature())?;
        }
        Ok(())
    })?;

    let ty = TypeStruct {
        ty: SType::Impl,
        ..ty
    };
    for function in &record.functions {
        create_fn_doc(w, env, function, Some(Box::new(ty.clone())), None)?;
    }
    Ok(())
}

fn create_enum_doc(w: &mut dyn Write, env: &Env, enum_: &Enumeration) -> Result<()> {
    let ty = enum_.to_stripper_type();
    let symbols = env.symbols.borrow();

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = enum_.doc {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &enum_.name))?;
        }
        if let Some(ver) = enum_.deprecated_version {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if enum_.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = enum_.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &enum_.name))?;
        }
        Ok(())
    })?;

    for member in &enum_.members {
        if member.doc.is_some() {
            let sub_ty = TypeStruct {
                name: member.name.to_camel(),
                parent: Some(Box::new(ty.clone())),
                ty: SType::Variant,
                args: Vec::new(),
            };
            write_item_doc(w, &sub_ty, |w| {
                if let Some(ref doc) = member.doc {
                    writeln!(w, "{}", reformat_doc(doc, &symbols, &enum_.name))?;
                }
                Ok(())
            })?;
        }
    }

    if let Some(version) = enum_.version {
        if version > env.config.min_cfg_version {
            writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
        }
    }
    Ok(())
}

fn create_bitfield_doc(w: &mut dyn Write, env: &Env, bitfield: &Bitfield) -> Result<()> {
    let ty = bitfield.to_stripper_type();
    let symbols = env.symbols.borrow();

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = bitfield.doc {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &bitfield.name))?;
        }
        if let Some(ver) = bitfield.deprecated_version {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if bitfield.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = bitfield.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols, &bitfield.name))?;
        }
        Ok(())
    })?;

    for member in &bitfield.members {
        if member.doc.is_some() {
            let sub_ty = TypeStruct {
                name: nameutil::bitfield_member_name(&member.name),
                parent: Some(Box::new(ty.clone())),
                ty: SType::Const,
                args: Vec::new(),
            };
            write_item_doc(w, &sub_ty, |w| {
                if let Some(ref doc) = member.doc {
                    writeln!(w, "{}", reformat_doc(doc, &symbols, &bitfield.name))?;
                }
                Ok(())
            })?;
        }
    }

    if let Some(version) = bitfield.version {
        if version > env.config.min_cfg_version {
            writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
        }
    }
    Ok(())
}

static PARAM_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"@(\w+)\b").unwrap());

fn fix_param_names<'a>(doc: &'a str, self_name: &Option<String>) -> Cow<'a, str> {
    PARAM_NAME.replace_all(doc, |caps: &Captures<'_>| {
        if let Some(ref self_name) = *self_name {
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
) -> Result<()>
where
    T: FunctionLikeType + ToStripperType,
{
    if env.is_totally_deprecated(*fn_.deprecated_version()) {
        return Ok(());
    }
    if fn_.doc().is_none()
        && fn_.doc_deprecated().is_none()
        && fn_.ret().doc.is_none()
        && fn_.parameters().iter().all(|p| p.doc.is_none())
    {
        return Ok(());
    }

    let parent_name = parent.as_ref().map_or("", |p| &p.name).to_owned();

    let symbols = env.symbols.borrow();
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
        if let Some(ref doc) = *fn_.doc() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols, &parent_name)
            )?;
        }
        if let Some(version) = *fn_.version() {
            if version > env.config.min_cfg_version {
                writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
            }
        }
        if let Some(ver) = *fn_.deprecated_version() {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if fn_.doc_deprecated().is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = *fn_.doc_deprecated() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols, &parent_name)
            )?;
        }

        for parameter in fn_.parameters() {
            if parameter.instance_parameter || parameter.name.is_empty() {
                continue;
            }
            if let Some(ref doc) = parameter.doc {
                writeln!(w, "## `{}`", nameutil::mangle_keywords(&parameter.name[..]))?;
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &self_name), &symbols, &parent_name)
                )?;
            }
        }

        if let Some(ref doc) = fn_.ret().doc {
            writeln!(w, "\n# Returns\n")?;
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols, &parent_name)
            )?;
        }
        Ok(())
    })
}

fn create_property_doc(
    w: &mut dyn Write,
    env: &Env,
    property: &Property,
    parent: Option<Box<TypeStruct>>,
) -> Result<()> {
    if env.is_totally_deprecated(property.deprecated_version) {
        return Ok(());
    }
    if property.doc.is_none()
        && property.doc_deprecated.is_none()
        && (property.readable || property.writable)
    {
        return Ok(());
    }
    let parent_name = parent.as_ref().map_or("", |p| &p.name).to_owned();
    let name_for_func = nameutil::signal_to_snake(&property.name);
    let mut v = Vec::with_capacity(2);

    let symbols = env.symbols.borrow();
    if property.readable {
        v.push(TypeStruct {
            parent: parent.clone(),
            ..TypeStruct::new(SType::Fn, &format!("get_property_{}", name_for_func))
        });
    }
    if property.writable {
        v.push(TypeStruct {
            parent,
            ..TypeStruct::new(SType::Fn, &format!("set_property_{}", name_for_func))
        });
    }

    for item in &v {
        write_item_doc(w, item, |w| {
            if let Some(ref doc) = property.doc {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), &symbols, &parent_name)
                )?;
            }
            if let Some(version) = property.version {
                if version > env.config.min_cfg_version {
                    writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
                }
            }
            if let Some(ver) = property.deprecated_version {
                writeln!(w, "\n# Deprecated since {}\n", ver)?;
            } else if property.doc_deprecated.is_some() {
                writeln!(w, "\n# Deprecated\n")?;
            }
            if let Some(ref doc) = property.doc_deprecated {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), &symbols, &parent_name)
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
        format!("[`{n}`](trait@crate::{n})", n = &trait_name)
    } else if let Some(symbol) = env.symbols.borrow().by_tid(tid) {
        format!("[`trait@{}{}`]", &symbol.parent(), trait_name)
    } else {
        error!("Type {} doesn't have crate", tid.full_name(&env.library));
        format!("`{}`", trait_name)
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
        .map(|name| format!("[`{n}`](trait@crate::{n})", n = name))
        .collect()
}
