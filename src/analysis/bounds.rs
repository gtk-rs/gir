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
    config::{self, parameter_matchable::ParameterMatchable},
    consts::TYPE_PARAMETERS_START,
    env::Env,
    library::{
        Basic, Class, Concurrency, Function, Nullable, ParameterDirection, ParameterScope, Type,
        TypeId,
    },
    traits::*,
};

#[derive(Clone, Copy, Eq, Debug, PartialEq)]
pub enum BoundType {
    NoWrapper,
    IsA,
    AsRef,
    IntoOption,
    IntoOptionRef,
    IntoOptionIsA,
}

impl BoundType {
    pub fn get_to_glib_extra(
        &self,
        nullable: bool,
        instance_parameter: bool,
        move_: bool,
    ) -> String {
        use BoundType::*;
        match *self {
            NoWrapper => "",
            AsRef if move_ && nullable => ".map(|p| p.as_ref().clone().upcast())",
            AsRef if nullable => ".as_ref().map(|p| p.as_ref())",
            AsRef if move_ => ".upcast()",
            AsRef => ".as_ref()",
            IsA if move_ && nullable => ".map(|p| p.upcast())",
            IsA if nullable && !instance_parameter => ".map(|p| p.as_ref())",
            IsA if move_ => ".upcast()",
            IsA => ".as_ref()",
            IntoOption | IntoOptionRef { .. } => ".into()",
            IntoOptionIsA if move_ => ".into()",
            IntoOptionIsA => ".into().as_ref().map(|p| p.as_ref())",
        }
        .to_string()
    }
}

#[derive(Clone, Eq, Debug, PartialEq)]
pub struct Bound {
    pub bound_type: BoundType,
    pub parameter_name: String,
    pub lt: Option<char>,
    pub alias: Option<char>,
    pub type_str: String,
    pub callback_modified: bool,
}

#[derive(Clone, Debug)]
pub struct Bounds {
    unused: VecDeque<char>,
    pub used: Vec<Bound>,
    pub lifetimes: Vec<char>,
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
        if par.direction == ParameterDirection::Out {
            return (None, None);
        }

        let nullable = configured_functions
            .matched_parameters(&par.name)
            .iter()
            .find_map(|p| p.nullable)
            .unwrap_or(par.nullable);
        let ref_mode = if par.move_ {
            RefMode::None
        } else {
            par.ref_mode
        };
        let Ok(rust_type) = RustType::builder(env, par.typ)
            .direction(par.direction)
            .nullable(nullable)
            .ref_mode(ref_mode)
            .try_build()
        else {
            return (None, None);
        };

        if par.instance_parameter {
            let ret = Bounds::get_to_glib_extra_for(
                env,
                par.typ,
                ref_mode,
                nullable,
                par.direction,
                par.instance_parameter,
                par.scope,
            );

            return (ret, None);
        }

        let mut type_string = rust_type.into_string();
        let mut ret = None;
        let mut callback_info = None;

        if let Some(mut bound_type) = Bounds::type_for(
            env,
            par.typ,
            ref_mode,
            nullable,
            par.direction,
            par.instance_parameter,
            par.scope,
        ) {
            ret = Some(bound_type.get_to_glib_extra(
                *par.nullable,
                par.instance_parameter,
                par.move_,
            ));

            if r#async && (par.name == "callback" || par.name.ends_with("_callback")) {
                bound_type = BoundType::NoWrapper;

                let func_name = func.c_identifier.as_ref().unwrap();
                let finish_func_name = if let Some(finish_func_name) = &func.finish_func {
                    finish_func_name.to_string()
                } else {
                    finish_function_name(func_name)
                };
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
                                .try_build_param()
                                .into_string(),
                        );
                    }
                    let parameters = format_out_parameters(&out_parameters);
                    let error_type = find_error_type(env, function);
                    if let Some(ref error) = error_type {
                        type_string = format!("FnOnce(Result<{parameters}, {error}>) + 'static");
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
            } else if par.c_type == "GDestroyNotify" || env.library.type_(par.typ).is_function() {
                if let Type::Function(_) = env.library.type_(par.typ) {
                    let callback_parameters_config = configured_functions.iter().find_map(|f| {
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
                        rust_ty = rust_ty.callback_parameters_config(callback_parameters_config);
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

            if par.c_type != "GDestroyNotify" {
                self.add_for(&par.name, &type_string, bound_type);
            }
        }

        (ret, callback_info)
    }

    pub fn add_for_property_setter(
        &mut self,
        env: &Env,
        type_id: TypeId,
        name: &str,
        ref_mode: RefMode,
        nullable: Nullable,
    ) -> bool {
        let Some(bound_type) = Bounds::type_for(
            env,
            type_id,
            ref_mode,
            nullable,
            ParameterDirection::In,
            false,
            ParameterScope::None,
        ) else {
            return false;
        };

        let type_str = RustType::builder(env, type_id)
            .nullable(nullable)
            .direction(ParameterDirection::In)
            .ref_mode(ref_mode)
            .try_build()
            .into_string();

        self.add_for(name, &type_str, bound_type);

        true
    }

    fn type_for(
        env: &Env,
        type_id: TypeId,
        ref_mode: RefMode,
        nullable: Nullable,
        direction: ParameterDirection,
        instance_parameter: bool,
        scope: ParameterScope,
    ) -> Option<BoundType> {
        match env.library.type_(type_id) {
            Type::Basic(Basic::Filename | Basic::OsString) => Some(BoundType::AsRef),
            Type::Class(Class {
                is_fundamental: true,
                ..
            }) => Some(BoundType::AsRef),
            Type::Class(Class {
                final_type: true, ..
            }) => {
                if *nullable
                    && direction == ParameterDirection::In
                    && !ref_mode.is_none()
                    && !instance_parameter
                {
                    Some(BoundType::IntoOptionRef)
                } else {
                    None
                }
            }
            Type::Class(Class {
                final_type: false, ..
            })
            | Type::Interface(..) => {
                if *nullable
                    && direction == ParameterDirection::In
                    && !ref_mode.is_none()
                    && !instance_parameter
                {
                    Some(BoundType::IntoOptionIsA)
                } else {
                    Some(BoundType::IsA)
                }
            }
            Type::List(_) | Type::SList(_) | Type::CArray(_) => None,
            Type::Function(_) => Some(BoundType::NoWrapper),
            _ => {
                if *nullable && direction == ParameterDirection::In && scope.is_none() {
                    if !ref_mode.is_none() {
                        Some(BoundType::IntoOptionRef)
                    } else {
                        Some(BoundType::IntoOption)
                    }
                } else {
                    None
                }
            }
        }
    }

    pub fn get_to_glib_extra_for(
        env: &Env,
        type_id: TypeId,
        ref_mode: RefMode,
        nullable: Nullable,
        direction: ParameterDirection,
        instance_parameter: bool,
        scope: ParameterScope,
    ) -> Option<String> {
        let bound_type = Self::type_for(
            env,
            type_id,
            ref_mode,
            nullable,
            direction,
            instance_parameter,
            scope,
        )?;

        let res = bound_type.get_to_glib_extra(*nullable, instance_parameter, ref_mode.is_none());

        if res.is_empty() {
            return None;
        }

        Some(res)
    }

    pub fn add_for(&mut self, name: &str, type_str: &str, bound_type: BoundType) {
        if self.used.iter().any(|n| n.parameter_name == name) {
            return;
        }

        let mut lt = None;
        let mut alias = None;
        match bound_type {
            BoundType::NoWrapper => {
                // TODO: This is just a heuristic for now, based on what we do in codegen!
                // Theoretically the surrounding function should determine whether it needs to
                // reuse an alias (ie. to use in `call_func::<P, Q, R>`) or not.
                // In the latter case an `impl` is generated instead of a type name/alias.
                alias = Some(self.unused.pop_front().expect("No free type aliases!"));
            }
            BoundType::IntoOptionRef => {
                lt = Some(self.get_lifetime());
            }
            BoundType::IntoOptionIsA => {
                lt = Some(self.get_lifetime());
                alias = Some(self.unused.pop_front().expect("No free type aliases!"));
            }
            _ => (),
        }

        self.used.push(Bound {
            bound_type,
            parameter_name: name.to_owned(),
            lt,
            alias,
            type_str: type_str.to_owned(),
            callback_modified: false,
        });
    }

    pub fn push(&mut self, bound: Bound) {
        if let Some(lt) = bound.lt {
            if !self.lifetimes.contains(&lt) {
                self.lifetimes.push(lt);
                self.lifetimes.sort();
            }
        }

        if let Some(alias) = bound.alias {
            if self
                .used
                .iter()
                .any(|b| b.alias.map_or(false, |a| a == alias))
            {
                panic!("Pushing a bound with an alias already in use");
            }

            self.unused.retain(|u| *u != alias);
        }

        self.used.push(bound);
        self.used.sort_by_key(|u| u.alias);
    }

    fn get_lifetime(&mut self) -> char {
        // In practice, we always need at most one lifetime so far
        let lt = 'a';
        if self.lifetimes.is_empty() {
            self.lifetimes.push(lt);
        }

        lt
    }

    pub fn get_parameter_bound(&self, name: &str) -> Option<&Bound> {
        self.iter().find(move |n| n.parameter_name == name)
    }

    pub fn update_imports(&self, imports: &mut Imports) {
        // TODO: import with versions
        use self::BoundType::*;
        for used in &self.used {
            match used.bound_type {
                NoWrapper { .. } | IntoOption | IntoOptionRef { .. } => (),
                IsA { .. } | IntoOptionIsA { .. } => imports.add("glib::prelude::*"),
                AsRef { .. } => imports.add_used_type(&used.type_str),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, Bound> {
        self.used.iter()
    }

    pub fn iter_aliases(&self) -> impl Iterator<Item = char> + '_ {
        self.used.iter().flat_map(|u| u.alias)
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
                .matched_parameters(&param.name)
                .iter()
                .find_map(|p| p.nullable)
                .unwrap_or(param.nullable);

            RustType::builder(env, param.typ)
                .direction(param.direction)
                .nullable(nullable)
                .try_build_param()
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
        Some(
            RustType::builder(env, error_param.typ)
                .direction(error_param.direction)
                .try_build()
                .into_string(),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::discriminant;

    #[test]
    fn get_new_all() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA;
        bounds.add_for("a", "", typ);
        assert_eq!(bounds.iter().len(), 1);
        // Don't add second time
        bounds.add_for("a", "", typ);
        assert_eq!(bounds.iter().len(), 1);
        bounds.add_for("b", "", typ);
        bounds.add_for("c", "", typ);
        bounds.add_for("d", "", typ);
        bounds.add_for("e", "", typ);
        bounds.add_for("f", "", typ);
        bounds.add_for("g", "", typ);
        bounds.add_for("h", "", typ);
        assert_eq!(bounds.iter().len(), 8);
        bounds.add_for("h", "", typ);
        assert_eq!(bounds.iter().len(), 8);
        bounds.add_for("i", "", typ);
        bounds.add_for("j", "", typ);
        bounds.add_for("k", "", typ);
    }

    #[test]
    #[should_panic(expected = "No free type aliases!")]
    fn exhaust_type_parameters() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::NoWrapper;
        for c in 'a'..='l' {
            // Should panic on `l` because all type parameters are exhausted
            bounds.add_for(c.to_string().as_str(), "", typ);
        }
    }

    #[test]
    fn get_parameter_bound() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::NoWrapper;
        bounds.add_for("a", "", typ);
        bounds.add_for("b", "", typ);
        let bound = bounds.get_parameter_bound("a").unwrap();
        // `NoWrapper `bounds are expected to have an alias:
        assert_eq!(bound.alias, Some('P'));
        assert_eq!(discriminant(&bound.bound_type), discriminant(&typ));
        let bound = bounds.get_parameter_bound("b").unwrap();
        assert_eq!(bound.alias, Some('Q'));
        assert_eq!(discriminant(&bound.bound_type), discriminant(&typ));
        assert_eq!(bounds.get_parameter_bound("c"), None);
    }

    #[test]
    fn impl_bound() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA;
        bounds.add_for("a", "", typ);
        bounds.add_for("b", "", typ);
        let bound = bounds.get_parameter_bound("a").unwrap();
        // `IsA` is simplified to an inline `foo: impl IsA<Bar>` and
        // lacks an alias/type-parameter:
        assert_eq!(bound.alias, None);
        assert_eq!(bound.bound_type, typ);

        let typ = BoundType::AsRef;
        bounds.add_for("c", "", typ);
        let bound = bounds.get_parameter_bound("c").unwrap();
        // Same `impl AsRef<Foo>` simplification as `IsA`:
        assert_eq!(bound.alias, None);
        assert_eq!(bound.bound_type, typ);
    }
}
