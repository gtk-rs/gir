use analysis::rust_type::*;
use analysis::types::*;
use codegen::sys::ffi_type::ffi_type;
use codegen::sys::functions::function_signature;
use env::Env;
use library::*;
use traits::IntoString;
use traits::MaybeRefAs;

pub struct Fields {
    /// Name of union, class, or a record that contains the fields.
    pub name: String,
    /// Is this external type?
    pub external: bool,
    /// Reason for truncating the representation, if any.
    pub truncated: Option<String>,
    derives_copy: bool,
    /// "struct" or "union"
    pub kind: &'static str,
    pub fields: Vec<FieldInfo>,
}

pub struct FieldInfo {
    /// Rust field name
    pub name: String,
    /// Rust type name
    pub typ: String,
    /// Does access to this field require unsafe block?
    unsafe_access: bool,
    /// Does this field implement debug trait?
    pub debug: bool,
}

impl Fields {
    /// List of derived traits
    pub fn derived_traits(&self) -> Vec<&'static str> {
        let mut traits = Vec::new();
        if self.derives_copy {
            traits.push("Copy");
            traits.push("Clone");
        }
        traits
    }
}

impl FieldInfo {
    /// Generates a string that accesses the field in the context of &self receiver.
    pub fn access_str(&self) -> String {
        let mut s = format!("&self.{}", self.name);
        if self.unsafe_access {
            s = format!("unsafe {{ {} }}", s);
        }
        s
    }
}

pub fn from_record(env: &Env, record: &Record) -> Fields {
    let (fields, truncated) = analyze_fields(env, false, &record.fields);
    Fields {
        name: record.c_type.clone(),
        external: record.is_external(&env.library),
        truncated,
        derives_copy: record.derives_copy(&env.library),
        kind: "struct",
        fields,
    }
}

pub fn from_class(env: &Env, klass: &Class) -> Fields {
    let (fields, truncated) = analyze_fields(env, false, &klass.fields);
    Fields {
        name: klass.c_type.clone(),
        external: klass.is_external(&env.library),
        truncated,
        derives_copy: klass.derives_copy(&env.library),
        kind: "struct",
        fields,
    }
}

pub fn from_union(env: &Env, union: &Union) -> Fields {
    let (fields, truncated) = analyze_fields(env, true, &union.fields);
    Fields {
        name: union.c_type.as_ref().unwrap().clone(),
        external: union.is_external(&env.library),
        truncated,
        derives_copy: union.derives_copy(&env.library),
        kind: "union",
        fields,
    }
}

fn analyze_fields(env: &Env, unsafe_access: bool, fields: &[Field]) -> (Vec<FieldInfo>, Option<String>) {
    let mut truncated = None;
    let mut infos = Vec::new();

    for field in fields {
        let typ = match field_ffi_type(env, field) {
            e @ Err(..) => {
                truncated = Some(e.into_string());
                break;
            }
            Ok(typ) => typ,
        };
        infos.push(FieldInfo {
            name: field.name.clone(),
            typ,
            debug: field.implements_debug(&env.library),
            unsafe_access,
        });
    }

    (infos, truncated)
}


fn field_ffi_type(env: &Env, field: &Field) -> Result {
    if field.is_incomplete(&env.library) {
        return Err(TypeError::Ignored(format!("field {} has incomplete type", &field.name)));
    }
    if let Some(ref c_type) = field.c_type {
        ffi_type(env, field.typ, c_type)
    } else if let Some(func) = env.library.type_(field.typ).maybe_ref_as::<Function>() {
        let (failure, signature) = function_signature(env, func, true);
        let signature = format!("Option<unsafe extern \"C\" fn{}>", signature);
        if failure {
            Err(TypeError::Unimplemented(signature))
        } else {
            Ok(signature)
        }
    } else {
        Err(TypeError::Ignored(format!("field {} has empty c:type", &field.name)))
    }
}
