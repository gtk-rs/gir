use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use chunk::{ffi_function_todo, Chunk};
use env::Env;

use writer::primitives::tabs;
use writer::ToCode;
use nameutil;

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::class_impls::SubclassInfo;


pub fn generate_impl(w: &mut Write,
                     env: &Env,
                     analysis: &analysis::functions::Info,
                     subclass_info: &SubclassInfo,
                     indent: usize
                 ) -> Result<()> {

    info!("{:?}", analysis.name);

    Ok(())
}
