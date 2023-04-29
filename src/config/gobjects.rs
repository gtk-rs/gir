use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
    sync::Arc,
};

use log::{error, warn};
use toml::Value;

use super::{
    child_properties::ChildProperties,
    constants::Constants,
    derives::Derives,
    functions::Functions,
    ident::Ident,
    members::Members,
    properties::Properties,
    signals::{Signal, Signals},
    virtual_methods::VirtualMethods,
};
use crate::{
    analysis::{conversion_type::ConversionType, ref_mode},
    codegen::Visibility,
    config::{
        error::TomlHelper,
        parsable::{Parsable, Parse},
    },
    library::{self, Library, TypeId, MAIN_NAMESPACE},
    version::Version,
};

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum GStatus {
    Manual,
    Generate,
    #[default]
    Ignore,
}

impl GStatus {
    pub fn ignored(self) -> bool {
        self == Self::Ignore
    }
    pub fn manual(self) -> bool {
        self == Self::Manual
    }
    pub fn need_generate(self) -> bool {
        self == Self::Generate
    }
}

impl FromStr for GStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "generate" => Ok(Self::Generate),
            "ignore" => Ok(Self::Ignore),
            e => Err(format!("Wrong object status: \"{e}\"")),
        }
    }
}

/// Info about `GObject` descendant
#[derive(Clone, Debug)]
pub struct GObject {
    pub name: String,
    pub functions: Functions,
    pub virtual_methods: VirtualMethods,
    pub constants: Constants,
    pub signals: Signals,
    pub members: Members,
    pub properties: Properties,
    pub derives: Option<Derives>,
    pub status: GStatus,
    pub module_name: Option<String>,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub type_id: Option<TypeId>,
    pub final_type: Option<bool>,
    pub fundamental_type: Option<bool>,
    pub exhaustive: bool,
    pub trait_name: Option<String>,
    pub child_properties: Option<ChildProperties>,
    pub concurrency: library::Concurrency,
    pub ref_mode: Option<ref_mode::RefMode>,
    pub must_use: bool,
    pub conversion_type: Option<ConversionType>,
    pub generate_display_trait: bool,
    pub trust_return_value_nullability: bool,
    pub manual_traits: Vec<String>,
    pub align: Option<u32>,
    pub generate_builder: bool,
    pub builder_postprocess: Option<String>,
    pub boxed_inline: bool,
    pub init_function_expression: Option<String>,
    pub copy_into_function_expression: Option<String>,
    pub clear_function_expression: Option<String>,
    pub visibility: Visibility,
    pub default_value: Option<String>,
    pub generate_doc: bool,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            functions: Functions::new(),
            virtual_methods: VirtualMethods::new(),
            constants: Constants::new(),
            signals: Signals::new(),
            members: Members::new(),
            properties: Properties::new(),
            derives: None,
            status: Default::default(),
            module_name: None,
            version: None,
            cfg_condition: None,
            type_id: None,
            final_type: None,
            fundamental_type: None,
            exhaustive: false,
            trait_name: None,
            child_properties: None,
            concurrency: Default::default(),
            ref_mode: None,
            must_use: false,
            conversion_type: None,
            generate_display_trait: true,
            trust_return_value_nullability: false,
            manual_traits: Vec::default(),
            align: None,
            generate_builder: false,
            builder_postprocess: None,
            boxed_inline: false,
            init_function_expression: None,
            copy_into_function_expression: None,
            clear_function_expression: None,
            visibility: Default::default(),
            default_value: None,
            generate_doc: true,
        }
    }
}

// TODO: ?change to HashMap<String, GStatus>
pub type GObjects = BTreeMap<String, GObject>;

pub fn parse_toml(
    toml_objects: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
    generate_builder: bool,
    trust_return_value_nullability: bool,
) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_array().unwrap() {
        let gobject = parse_object(
            toml_object,
            concurrency,
            generate_display_trait,
            generate_builder,
            trust_return_value_nullability,
        );
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

pub fn parse_conversion_type(toml: Option<&Value>, object_name: &str) -> Option<ConversionType> {
    use crate::analysis::conversion_type::ConversionType::*;

    let v = toml?;
    v.check_unwanted(&["variant", "ok_type", "err_type"], "conversion_type");

    let (conversion_type, ok_type, err_type) = match &v {
        Value::Table(table) => {
            let conversion_type = table.get("variant").and_then(Value::as_str);
            if conversion_type.is_none() {
                error!("Missing `variant` for {}.conversion_type", object_name);
                return None;
            }

            let ok_type = Some(Arc::from(
                table
                    .get("ok_type")
                    .and_then(Value::as_str)
                    .unwrap_or(object_name),
            ));
            let err_type = table.get("err_type").and_then(Value::as_str);

            (conversion_type.unwrap(), ok_type, err_type)
        }
        Value::String(conversion_type) => (conversion_type.as_str(), None, None),
        _ => {
            error!("Unexpected toml item for {}.conversion_type", object_name);
            return None;
        }
    };

    let get_err_type = || -> Arc<str> {
        err_type.map_or_else(
            || {
                error!("Missing `err_type` for {}.conversion_type", object_name);
                Arc::from("MissingErrorType")
            },
            Arc::from,
        )
    };

    match conversion_type {
        "direct" => Some(Direct),
        "scalar" => Some(Scalar),
        "Option" => Some(Option),
        "Result" => Some(Result {
            ok_type: ok_type.expect("Missing `ok_type`"),
            err_type: get_err_type(),
        }),
        "pointer" => Some(Pointer),
        "borrow" => Some(Borrow),
        "unknown" => Some(Unknown),
        unexpected => {
            error!(
                "Unexpected {} for {}.conversion_type",
                unexpected, object_name
            );
            None
        }
    }
}

fn parse_object(
    toml_object: &Value,
    concurrency: library::Concurrency,
    default_generate_display_trait: bool,
    generate_builder: bool,
    trust_return_value_nullability: bool,
) -> GObject {
    let name: String = toml_object
        .lookup("name")
        .expect("Object name not defined")
        .as_str()
        .unwrap()
        .into();
    // Also checks for ChildProperties
    toml_object.check_unwanted(
        &[
            "name",
            "status",
            "function",
            "constant",
            "signal",
            "member",
            "property",
            "derive",
            "module_name",
            "version",
            "concurrency",
            "ref_mode",
            "conversion_type",
            "child_prop",
            "child_name",
            "child_type",
            "final_type",
            "fundamental_type",
            "exhaustive",
            "trait",
            "trait_name",
            "cfg_condition",
            "must_use",
            "generate_display_trait",
            "trust_return_value_nullability",
            "manual_traits",
            "align",
            "generate_builder",
            "builder_postprocess",
            "boxed_inline",
            "init_function_expression",
            "copy_into_function_expression",
            "clear_function_expression",
            "visibility",
            "default_value",
            "generate_doc",
        ],
        &format!("object {name}"),
    );

    let status = match toml_object.lookup("status") {
        Some(value) => {
            GStatus::from_str(value.as_str().unwrap()).unwrap_or_else(|_| Default::default())
        }
        None => Default::default(),
    };

    let constants = Constants::parse(toml_object.lookup("constant"), &name);
    let functions = Functions::parse(toml_object.lookup("function"), &name);
    let mut function_names = HashSet::new();
    for f in &functions {
        if let Ident::Name(name) = &f.ident {
            assert!(function_names.insert(name), "{name} already defined!");
        }
    }
    let virtual_methods = VirtualMethods::parse(toml_object.lookup("virtual_method"), &name);
    let mut virtual_methods_names = HashSet::new();
    for f in &virtual_methods {
        if let Ident::Name(name) = &f.ident {
            assert!(
                virtual_methods_names.insert(name),
                "{name} already defined!"
            );
        }
    }

    let signals = {
        let mut v = Vec::new();
        if let Some(configs) = toml_object.lookup("signal").and_then(Value::as_array) {
            for config in configs {
                if let Some(item) = Signal::parse(config, &name, concurrency) {
                    v.push(item);
                }
            }
        }

        v
    };
    let members = Members::parse(toml_object.lookup("member"), &name);
    let properties = Properties::parse(toml_object.lookup("property"), &name);
    let derives = toml_object
        .lookup("derive")
        .map(|derives| Derives::parse(Some(derives), &name));
    let module_name = toml_object
        .lookup("module_name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let version = toml_object
        .lookup("version")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok());
    let cfg_condition = toml_object
        .lookup("cfg_condition")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let generate_trait = toml_object.lookup("trait").and_then(Value::as_bool);
    let final_type = toml_object
        .lookup("final_type")
        .and_then(Value::as_bool)
        .or_else(|| generate_trait.map(|t| !t));
    let fundamental_type = toml_object
        .lookup("fundamental_type")
        .and_then(Value::as_bool);
    let exhaustive = toml_object
        .lookup("exhaustive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trait_name = toml_object
        .lookup("trait_name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let concurrency = toml_object
        .lookup("concurrency")
        .and_then(Value::as_str)
        .and_then(|v| v.parse().ok())
        .unwrap_or(concurrency);
    let ref_mode = toml_object
        .lookup("ref_mode")
        .and_then(Value::as_str)
        .and_then(|v| v.parse().ok());
    let conversion_type = parse_conversion_type(toml_object.lookup("conversion_type"), &name);
    let child_properties = ChildProperties::parse(toml_object, &name);
    let must_use = toml_object
        .lookup("must_use")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let generate_display_trait = toml_object
        .lookup("generate_display_trait")
        .and_then(Value::as_bool)
        .unwrap_or(default_generate_display_trait);
    let trust_return_value_nullability = toml_object
        .lookup("trust_return_value_nullability")
        .and_then(Value::as_bool)
        .unwrap_or(trust_return_value_nullability);
    let manual_traits = toml_object
        .lookup_vec("manual_traits", "IGNORED ERROR")
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let align = toml_object
        .lookup("align")
        .and_then(Value::as_integer)
        .and_then(|v| {
            if v.count_ones() != 1 || v > i64::from(u32::max_value()) || v < 0 {
                warn!(
                    "`align` configuration must be a power of two of type u32, found {}",
                    v
                );
                None
            } else {
                Some(v as u32)
            }
        });
    let generate_builder = toml_object
        .lookup("generate_builder")
        .and_then(Value::as_bool)
        .unwrap_or(generate_builder);

    let boxed_inline = toml_object
        .lookup("boxed_inline")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let builder_postprocess = toml_object
        .lookup("builder_postprocess")
        .and_then(Value::as_str)
        .map(String::from);
    let init_function_expression = toml_object
        .lookup("init_function_expression")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let copy_into_function_expression = toml_object
        .lookup("copy_into_function_expression")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let clear_function_expression = toml_object
        .lookup("clear_function_expression")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let default_value = toml_object
        .lookup("default_value")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let visibility = toml_object
        .lookup("visibility")
        .and_then(Value::as_str)
        .map(|v| v.parse())
        .transpose();
    if let Err(ref err) = visibility {
        error!("{}", err);
    }
    let visibility = visibility.ok().flatten().unwrap_or_default();
    if boxed_inline
        && !((init_function_expression.is_none()
            && copy_into_function_expression.is_none()
            && clear_function_expression.is_none())
            || (init_function_expression.is_some()
                && copy_into_function_expression.is_some()
                && clear_function_expression.is_some()))
    {
        panic!(
            "`init_function_expression`, `copy_into_function_expression` and `clear_function_expression` all have to be provided or neither"
        );
    }

    if !boxed_inline
        && (init_function_expression.is_some()
            || copy_into_function_expression.is_some()
            || clear_function_expression.is_some())
    {
        panic!(
            "`init_function_expression`, `copy_into_function_expression` and `clear_function_expression` can only be provided for BoxedInline types"
        );
    }

    if status != GStatus::Manual && ref_mode.is_some() {
        warn!("ref_mode configuration used for non-manual object {}", name);
    }

    if status != GStatus::Manual
        && !conversion_type
            .as_ref()
            .map_or(true, ConversionType::can_use_to_generate)
    {
        warn!(
            "unexpected conversion_type {:?} configuration used for non-manual object {}",
            conversion_type, name
        );
    }

    let generate_doc = toml_object
        .lookup("generate_doc")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if generate_trait.is_some() {
        warn!(
            "`trait` configuration is deprecated and replaced by `final_type` for object {}",
            name
        );
    }

    GObject {
        name,
        functions,
        virtual_methods,
        constants,
        signals,
        members,
        properties,
        derives,
        status,
        module_name,
        version,
        cfg_condition,
        type_id: None,
        final_type,
        fundamental_type,
        exhaustive,
        trait_name,
        child_properties,
        concurrency,
        ref_mode,
        must_use,
        conversion_type,
        generate_display_trait,
        trust_return_value_nullability,
        manual_traits,
        align,
        generate_builder,
        builder_postprocess,
        boxed_inline,
        init_function_expression,
        copy_into_function_expression,
        clear_function_expression,
        visibility,
        default_value,
        generate_doc,
    }
}

pub fn parse_status_shorthands(
    objects: &mut GObjects,
    toml: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
    generate_builder: bool,
    trust_return_value_nullability: bool,
) {
    use self::GStatus::*;
    for &status in &[Manual, Generate, Ignore] {
        parse_status_shorthand(
            objects,
            status,
            toml,
            concurrency,
            generate_display_trait,
            generate_builder,
            trust_return_value_nullability,
        );
    }
}

fn parse_status_shorthand(
    objects: &mut GObjects,
    status: GStatus,
    toml: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
    generate_builder: bool,
    trust_return_value_nullability: bool,
) {
    let option_name = format!("options.{status:?}").to_ascii_lowercase();
    if let Some(a) = toml.lookup(&option_name).map(|a| a.as_array().unwrap()) {
        for name in a.iter().map(|s| s.as_str().unwrap()) {
            match objects.get(name) {
                None => {
                    objects.insert(
                        name.into(),
                        GObject {
                            name: name.into(),
                            status,
                            concurrency,
                            generate_display_trait,
                            trust_return_value_nullability,
                            generate_builder,
                            ..Default::default()
                        },
                    );
                }
                Some(_) => panic!("Bad name in {option_name}: {name} already defined"),
            }
        }
    }
}

pub fn resolve_type_ids(objects: &mut GObjects, library: &Library) {
    let ns = library.namespace(MAIN_NAMESPACE);
    let global_functions_name = format!("{}.*", ns.name);

    for (name, object) in objects.iter_mut() {
        let type_id = library.find_type(0, name);
        if type_id.is_none() && name != &global_functions_name && object.status != GStatus::Ignore {
            warn!("Configured object `{}` missing from the library", name);
        } else if object.generate_builder {
            if let Some(type_id) = type_id {
                if library.type_(type_id).is_abstract() {
                    warn!(
                        "Cannot generate builder for `{}` because it's a base class",
                        name
                    );
                    // We set this to `false` to avoid having the "not_bound" mode saying that this
                    // builder should be generated.
                    object.generate_builder = false;
                }
            }
        }
        object.type_id = type_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analysis::conversion_type::ConversionType, library::Concurrency};

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn conversion_type_default() {
        let toml = &toml(
            r#"
name = "Test"
status = "generate"
"#,
        );

        let object = parse_object(toml, Concurrency::default(), false, false, false);
        assert_eq!(object.conversion_type, None);
    }

    #[test]
    fn conversion_type_option_str() {
        let toml = toml(
            r#"
name = "Test"
status = "generate"
conversion_type = "Option"
"#,
        );

        let object = parse_object(&toml, Concurrency::default(), false, false, false);
        assert_eq!(object.conversion_type, Some(ConversionType::Option));
    }

    #[test]
    fn conversion_type_option_table() {
        let toml = &toml(
            r#"
name = "Test"
status = "generate"
    [conversion_type]
        variant = "Option"
"#,
        );

        let object = parse_object(toml, Concurrency::default(), false, false, false);
        assert_eq!(object.conversion_type, Some(ConversionType::Option));
    }

    #[test]
    fn conversion_type_result_table_missing_err() {
        let toml = &toml(
            r#"
name = "Test"
status = "generate"
    [conversion_type]
        variant = "Result"
"#,
        );

        let object = parse_object(toml, Concurrency::default(), false, false, false);
        assert_eq!(
            object.conversion_type,
            Some(ConversionType::Result {
                ok_type: Arc::from("Test"),
                err_type: Arc::from("MissingErrorType"),
            }),
        );
    }

    #[test]
    fn conversion_type_result_table_with_err() {
        let toml = &toml(
            r#"
name = "Test"
status = "generate"
    [conversion_type]
        variant = "Result"
        err_type = "TryFromIntError"
"#,
        );

        let object = parse_object(toml, Concurrency::default(), false, false, false);
        assert_eq!(
            object.conversion_type,
            Some(ConversionType::Result {
                ok_type: Arc::from("Test"),
                err_type: Arc::from("TryFromIntError"),
            }),
        );
    }

    #[test]
    fn conversion_type_result_table_with_ok_err() {
        let toml = &toml(
            r#"
name = "Test"
status = "generate"
    [conversion_type]
        variant = "Result"
        ok_type = "TestSuccess"
        err_type = "TryFromIntError"
"#,
        );

        let object = parse_object(toml, Concurrency::default(), false, false, false);
        assert_eq!(
            object.conversion_type,
            Some(ConversionType::Result {
                ok_type: Arc::from("TestSuccess"),
                err_type: Arc::from("TryFromIntError"),
            }),
        );
    }

    #[test]
    fn conversion_type_fields() {
        let toml = &toml(
            r#"
[[object]]
name = "Test"
status = "generate"
    [[object.constant]]
    name = "Const"
    [[object.function]]
    name = "Func"
    manual = true

"#,
        );

        let object = toml
            .lookup("object")
            .map(|t| parse_toml(t, Concurrency::default(), false, false, false))
            .expect("parsing failed");
        assert_eq!(
            object["Test"].constants,
            vec![crate::config::constants::Constant {
                ident: Ident::Name("Const".to_owned()),
                status: GStatus::Generate,
                version: None,
                cfg_condition: None,
                generate_doc: true,
            }],
        );
        assert_eq!(object["Test"].functions.len(), 1);
        assert_eq!(
            object["Test"].functions[0].ident,
            Ident::Name("Func".to_owned()),
        );
    }

    #[test]
    fn conversion_type_generate_doc() {
        let r = &toml(
            r#"
name = "Test"
status = "generate"
generate_doc = false
"#,
        );

        let object = parse_object(r, Concurrency::default(), false, false, false);
        assert!(!object.generate_doc);

        // Ensure that the default value is "true".
        let r = &toml(
            r#"
name = "Test"
status = "generate"
"#,
        );
        let object = parse_object(r, Concurrency::default(), false, false, false);
        assert!(object.generate_doc);
    }
}
