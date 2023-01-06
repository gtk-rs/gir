use std::io::{Result, Write};

use super::general::version_condition;
use crate::{
    analysis::{self, special_functions::FunctionType},
    version::Version,
    Env,
};

pub(super) fn generate(
    w: &mut dyn Write,
    env: &Env,
    function: &analysis::functions::Info,
    specials: &analysis::special_functions::Infos,
    scope_version: Option<Version>,
) -> Result<bool> {
    if let Some(special) = specials.functions().get(&function.glib_name) {
        match special.type_ {
            FunctionType::StaticStringify => {
                generate_static_to_str(w, env, function, scope_version)
            }
        }
        .map(|()| true)
    } else {
        Ok(false)
    }
}

pub(super) fn generate_static_to_str(
    w: &mut dyn Write,
    env: &Env,
    function: &analysis::functions::Info,
    scope_version: Option<Version>,
) -> Result<()> {
    writeln!(w)?;
    let version = Version::if_stricter_than(function.version, scope_version);
    version_condition(w, env, None, version, false, 1)?;

    writeln!(
        w,
        "\
\t{visibility} fn {rust_fn_name}<'a>(self) -> &'a GStr {{
\t\tunsafe {{
\t\t\tGStr::from_ptr(
\t\t\t\t{ns}::{glib_fn_name}(self.into_glib())
\t\t\t\t\t.as_ref()
\t\t\t\t\t.expect(\"{glib_fn_name} returned NULL\"),
\t\t\t)
\t\t}}
\t}}",
        visibility = function.visibility,
        rust_fn_name = function.codegen_name(),
        ns = env.main_sys_crate_name(),
        glib_fn_name = function.glib_name,
    )?;

    Ok(())
}
