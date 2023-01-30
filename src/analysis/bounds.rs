use std::{collections::vec_deque::VecDeque, slice::Iter};

use crate::{
    analysis::{
        function_parameters::CParameter,
        functions::{find_function, find_index_to_ignore, finish_function_name},
        imports::Imports,
        out_parameters::use_function_return_for_result,
        ref_mode::RefMode,
        rust_type::RustType,
    },
    config,
    consts::TYPE_PARAMETERS_START,
    env::Env,
    library::{Basic, Class, Concurrency, Function, ParameterDirection, Type, TypeId},
    traits::IntoString,
};

#[derive(Clone, Eq, Debug, PartialEq)]
pub enum BoundType {
    NoWrapper,
    // lifetime
    IsA(Option<char>),
    // lifetime <- shouldn't be used but just in case...
    AsRef(Option<char>),
}

impl BoundType {
    pub fn need_isa(&self) -> bool {
        matches!(*self, Self::IsA(_))
    }

    // TODO: This is just a heuristic for now, based on what we do in codegen!
    // Theoretically the surrounding function should determine whether it needs to
    // reuse an alias (ie. to use in `call_func::<P, Q, R>`) or not.
    // In the latter case an `impl` is generated instead of a type name/alias.
    pub fn has_alias(&self) -> bool {
        matches!(*self, Self::NoWrapper)
    }
}

#[derive(Clone, Eq, Debug, PartialEq)]
pub struct Bound {
    pub bound_type: BoundType,
    pub parameter_name: String,
    /// Bound does not have an alias when `param: impl type_str` is used
    pub alias: Option<char>,
    pub type_str: String,
    pub callback_modified: bool,
}

#[derive(Clone, Debug)]
pub struct Bounds {
    unused: VecDeque<char>,
    used: Vec<Bound>,
    lifetimes: Vec<char>,
}

impl Default for Bounds {
    fn default() -> Bounds {
        Bounds {
            unused: (TYPE_PARAMETERS_START..)
                .take_while(|x| *x <= 'Z')
                .collect(),
            used: Vec::new(),
            lifetimes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallbackInfo {
    pub callback_type: String,
    pub success_parameters: String,
    pub error_parameters: Option<String>,
    pub bound_name: char,
}

impl Bounds {
    pub fn add_for_parameter(
        &mut self,
        env: &Env,
        func: &Function,
        par: &CParameter,
        r#async: bool,
        concurrency: Concurrency,
        configured_functions: &[&config::functions::Function],
    ) -> (Option<String>, Option<CallbackInfo>) {
        let type_name = RustType::builder(env, par.typ)
            .ref_mode(RefMode::ByRefFake)
            .try_build();
        if type_name.is_err() {
            return (None, None);
        }
        let mut type_string = type_name.into_string();
        let mut callback_info = None;
        let mut ret = None;
        let mut need_is_into_check = false;

        if !par.instance_parameter && par.direction != ParameterDirection::Out {
            if let Some(bound_type) = Bounds::type_for(env, par.typ) {
                ret = Some(Bounds::get_to_glib_extra(
                    &bound_type,
                    *par.nullable,
                    par.instance_parameter,
                    par.move_,
                ));
                if r#async && (par.name == "callback" || par.name.ends_with("_callback")) {
                    let func_name = func.c_identifier.as_ref().unwrap();
                    let finish_func_name = finish_function_name(func_name);
                    if let Some(function) = find_function(env, &finish_func_name) {
                        // FIXME: This should work completely based on the analysis of the finish()
                        // function but that a) happens afterwards and b) is
                        // not accessible from here either.
                        let mut out_parameters =
                            find_out_parameters(env, function, configured_functions);
                        if use_function_return_for_result(
                            env,
                            function.ret.typ,
                            &func.name,
                            configured_functions,
                        ) {
                            let nullable = configured_functions
                                .iter()
                                .find_map(|f| f.ret.nullable)
                                .unwrap_or(function.ret.nullable);

                            out_parameters.insert(
                                0,
                                RustType::builder(env, function.ret.typ)
                                    .direction(function.ret.direction)
                                    .nullable(nullable)
                                    .try_build()
                                    .into_string(),
                            );
                        }
                        let parameters = format_out_parameters(&out_parameters);
                        let error_type = find_error_type(env, function);
                        if let Some(ref error) = error_type {
                            type_string =
                                format!("FnOnce(Result<{parameters}, {error}>) + 'static");
                        } else {
                            type_string = format!("FnOnce({parameters}) + 'static");
                        }
                        let bound_name = *self.unused.front().unwrap();
                        callback_info = Some(CallbackInfo {
                            callback_type: type_string.clone(),
                            success_parameters: parameters,
                            error_parameters: error_type,
                            bound_name,
                        });
                    }
                } else if par.c_type == "GDestroyNotify" || env.library.type_(par.typ).is_function()
                {
                    need_is_into_check = par.c_type != "GDestroyNotify";
                    if let Type::Function(_) = env.library.type_(par.typ) {
                        let callback_parameters_config =
                            configured_functions.iter().find_map(|f| {
                                f.parameters
                                    .iter()
                                    .find(|p| p.ident.is_match(&par.name))
                                    .map(|p| &p.callback_parameters)
                            });

                        let mut rust_ty = RustType::builder(env, par.typ)
                            .direction(par.direction)
                            .scope(par.scope)
                            .concurrency(concurrency);
                        if let Some(callback_parameters_config) = callback_parameters_config {
                            rust_ty =
                                rust_ty.callback_parameters_config(callback_parameters_config);
                        }
                        type_string = rust_ty
                            .try_from_glib(&par.try_from_glib)
                            .try_build()
                            .into_string();
                        let bound_name = *self.unused.front().unwrap();
                        callback_info = Some(CallbackInfo {
                            callback_type: type_string.clone(),
                            success_parameters: String::new(),
                            error_parameters: None,
                            bound_name,
                        });
                    }
                }
                if (!need_is_into_check || !*par.nullable) && par.c_type != "GDestroyNotify" {
                    self.add_parameter(&par.name, &type_string, bound_type, r#async);
                }
            }
        } else if par.instance_parameter {
            if let Some(bound_type) = Bounds::type_for(env, par.typ) {
                ret = Some(Bounds::get_to_glib_extra(
                    &bound_type,
                    *par.nullable,
                    true,
                    par.move_,
                ));
            }
        }

        (ret, callback_info)
    }

    pub fn type_for(env: &Env, type_id: TypeId) -> Option<BoundType> {
        use self::BoundType::*;
        match env.library.type_(type_id) {
            Type::Basic(Basic::Filename | Basic::OsString) => Some(AsRef(None)),
            Type::Class(Class {
                is_fundamental: true,
                ..
            }) => Some(AsRef(None)),
            Type::Class(Class {
                final_type: true, ..
            }) => None,
            Type::Class(Class {
                final_type: false, ..
            }) => Some(IsA(None)),
            Type::Interface(..) => Some(IsA(None)),
            Type::List(_) | Type::SList(_) | Type::CArray(_) => None,
            Type::Function(_) => Some(NoWrapper),
            _ => None,
        }
    }

    pub fn get_to_glib_extra(
        bound_type: &BoundType,
        nullable: bool,
        instance_parameter: bool,
        move_: bool,
    ) -> String {
        use self::BoundType::*;
        match bound_type {
            AsRef(_) if move_ && nullable => ".map(|p| p.as_ref().clone().upcast())".to_owned(),
            AsRef(_) if nullable => ".as_ref().map(|p| p.as_ref())".to_owned(),
            AsRef(_) if move_ => ".upcast()".to_owned(),
            AsRef(_) => ".as_ref()".to_owned(),
            IsA(_) if move_ && nullable => ".map(|p| p.upcast())".to_owned(),
            IsA(_) if nullable && !instance_parameter => ".map(|p| p.as_ref())".to_owned(),
            IsA(_) if move_ => ".upcast()".to_owned(),
            IsA(_) => ".as_ref()".to_owned(),
            _ => String::new(),
        }
    }

    pub fn add_parameter(
        &mut self,
        name: &str,
        type_str: &str,
        mut bound_type: BoundType,
        r#async: bool,
    ) {
        if r#async && name == "callback" {
            bound_type = BoundType::NoWrapper;
        }
        if self.used.iter().any(|n| n.parameter_name == name) {
            return;
        }
        let alias = bound_type
            .has_alias()
            .then(|| self.unused.pop_front().expect("No free type aliases!"));
        self.used.push(Bound {
            bound_type,
            parameter_name: name.to_owned(),
            alias,
            type_str: type_str.to_owned(),
            callback_modified: false,
        });
    }

    pub fn get_parameter_bound(&self, name: &str) -> Option<&Bound> {
        self.iter().find(move |n| n.parameter_name == name)
    }

    pub fn update_imports(&self, imports: &mut Imports) {
        // TODO: import with versions
        use self::BoundType::*;
        for used in &self.used {
            match used.bound_type {
                NoWrapper => (),
                IsA(_) => imports.add("glib::prelude::*"),
                AsRef(_) => imports.add_used_type(&used.type_str),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, Bound> {
        self.used.iter()
    }

    pub fn iter_lifetimes(&self) -> Iter<'_, char> {
        self.lifetimes.iter()
    }
}

#[derive(Clone, Debug)]
pub struct PropertyBound {
    pub alias: char,
    pub type_str: String,
}

impl PropertyBound {
    pub fn get(env: &Env, type_id: TypeId) -> Option<Self> {
        let type_ = env.type_(type_id);
        if type_.is_final_type() {
            return None;
        }
        Some(Self {
            alias: TYPE_PARAMETERS_START,
            type_str: RustType::builder(env, type_id)
                .ref_mode(RefMode::ByRefFake)
                .try_build()
                .into_string(),
        })
    }
}

fn find_out_parameters(
    env: &Env,
    function: &Function,
    configured_functions: &[&config::functions::Function],
) -> Vec<String> {
    let index_to_ignore = find_index_to_ignore(&function.parameters, Some(&function.ret));
    function
        .parameters
        .iter()
        .enumerate()
        .filter(|&(index, param)| {
            Some(index) != index_to_ignore
                && param.direction == ParameterDirection::Out
                && param.name != "error"
        })
        .map(|(_, param)| {
            // FIXME: This should work completely based on the analysis of the finish()
            // function but that a) happens afterwards and b) is not accessible
            // from here either.
            let nullable = configured_functions
                .iter()
                .find_map(|f| {
                    f.parameters
                        .iter()
                        .filter(|p| p.ident.is_match(&param.name))
                        .find_map(|p| p.nullable)
                })
                .unwrap_or(param.nullable);

            RustType::builder(env, param.typ)
                .direction(param.direction)
                .nullable(nullable)
                .try_build()
                .into_string()
        })
        .collect()
}

fn format_out_parameters(parameters: &[String]) -> String {
    if parameters.len() == 1 {
        parameters[0].to_string()
    } else {
        format!("({})", parameters.join(", "))
    }
}

fn find_error_type(env: &Env, function: &Function) -> Option<String> {
    let error_param = function
        .parameters
        .iter()
        .find(|param| param.direction.is_out() && param.is_error)?;
    if let Type::Record(_) = env.type_(error_param.typ) {
        return Some(
            RustType::builder(env, error_param.typ)
                .direction(error_param.direction)
                .try_build()
                .into_string(),
        );
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_new_all() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA(None);
        bounds.add_parameter("a", "", typ.clone(), false);
        assert_eq!(bounds.iter().len(), 1);
        // Don't add second time
        bounds.add_parameter("a", "", typ.clone(), false);
        assert_eq!(bounds.iter().len(), 1);
        bounds.add_parameter("b", "", typ.clone(), false);
        bounds.add_parameter("c", "", typ.clone(), false);
        bounds.add_parameter("d", "", typ.clone(), false);
        bounds.add_parameter("e", "", typ.clone(), false);
        bounds.add_parameter("f", "", typ.clone(), false);
        bounds.add_parameter("g", "", typ.clone(), false);
        bounds.add_parameter("h", "", typ.clone(), false);
        assert_eq!(bounds.iter().len(), 8);
        bounds.add_parameter("h", "", typ.clone(), false);
        assert_eq!(bounds.iter().len(), 8);
        bounds.add_parameter("i", "", typ.clone(), false);
        bounds.add_parameter("j", "", typ.clone(), false);
        bounds.add_parameter("k", "", typ, false);
    }

    #[test]
    #[should_panic(expected = "No free type aliases!")]
    fn exhaust_type_parameters() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::NoWrapper;
        for c in 'a'..='l' {
            // Should panic on `l` because all type parameters are exhausted
            bounds.add_parameter(c.to_string().as_str(), "", typ.clone(), false);
        }
    }

    #[test]
    fn get_parameter_bound() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::NoWrapper;
        bounds.add_parameter("a", "", typ.clone(), false);
        bounds.add_parameter("b", "", typ.clone(), false);
        let bound = bounds.get_parameter_bound("a").unwrap();
        // `NoWrapper `bounds are expected to have an alias:
        assert_eq!(bound.alias, Some('P'));
        assert_eq!(bound.bound_type, typ);
        let bound = bounds.get_parameter_bound("b").unwrap();
        assert_eq!(bound.alias, Some('Q'));
        assert_eq!(bound.bound_type, typ);
        assert_eq!(bounds.get_parameter_bound("c"), None);
    }

    #[test]
    fn impl_bound() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA(None);
        bounds.add_parameter("a", "", typ.clone(), false);
        bounds.add_parameter("b", "", typ.clone(), false);
        let bound = bounds.get_parameter_bound("a").unwrap();
        // `IsA` is simplified to an inline `foo: impl IsA<Bar>` and
        // lacks an alias/type-parameter:
        assert_eq!(bound.alias, None);
        assert_eq!(bound.bound_type, typ);

        let typ = BoundType::AsRef(None);
        bounds.add_parameter("c", "", typ.clone(), false);
        let bound = bounds.get_parameter_bound("c").unwrap();
        // Same `impl AsRef<Foo>` simplification as `IsA`:
        assert_eq!(bound.alias, None);
        assert_eq!(bound.bound_type, typ);
    }
}
