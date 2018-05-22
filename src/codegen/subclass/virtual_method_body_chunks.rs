use analysis::conversion_type::ConversionType;
use analysis::functions::{AsyncTrampoline, find_index_to_ignore};
use analysis::function_parameters::CParameter as AnalysisCParameter;
use analysis::function_parameters::{Transformation, TransformationType};
use analysis::out_parameters::Mode;
use analysis::namespaces;
use analysis::return_value;
use analysis::rust_type::rust_type;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use analysis;
use chunk::{Chunk, Param, TupleMode};
use chunk::parameter_ffi_call_out;
use env::Env;
use library::{self, ParameterDirection};
use nameutil;


#[derive(Default)]
pub struct Builder {
    object_class_c_type: String,
    ffi_crate_name: String
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }

    pub fn object_class_c_type(&mut self, c_class_type: &str) -> &mut Builder {
        self.object_class_c_type = c_class_type.into();
        self
    }

    pub fn ffi_crate_name(&mut self, ns: &str) -> &mut Builder {
        self.ffi_crate_name = ns.into();
        self
    }
    //
    // pub fn in_trait(&mut self, value: bool) -> &mut Builder {
    //     self.in_trait = value;
    //     self
    // }
    //
    // pub fn function_type_string(&mut self, type_: &str) -> &mut Builder {
    //     self.function_type_string = type_.into();
    //     self
    // }

    pub fn generate(&self, env: &Env) -> Chunk {
        let mut body = Vec::new();


        body.push(self.let_klass());
        body.push(self.let_parent_klass());


        // fn parent_startup(&self) {
        //     unsafe {
        //         let klass = self.get_class();
        //         let parent_klass = (*klass).get_parent_class() as *const gio_ffi::GApplicationClass;
        //         (*parent_klass)
        //             .startup
        //             .map(|f| f(self.to_glib_none().0))
        //             .unwrap_or(())
        //     }
        // }


        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();
        chunks.push(unsafe_);
        Chunk::Chunks(chunks)
    }

    fn let_klass(&self) -> Chunk {
        Chunk::Let {
            name: "klass".to_owned(),
            is_mut: false,
            value: Box::new(Chunk::Custom("self.get_class()".to_owned())),
            type_: None
        }
    }

    fn let_parent_klass(&self) -> Chunk {
        Chunk::Let {
            name: "parent_klass".to_owned(),
            is_mut: false,
            value: Box::new(
                Chunk::Cast {
                    name: "(*klass).get_parent_class()".to_owned(),
                    type_: format!("*const {}::{}", self.ffi_crate_name, self.object_class_c_type).to_owned()
                }),
            type_: None
        }
    }

    //let parent_klass = (*klass).get_parent_class() as *const gio_ffi::GApplicationClass;

    // fn connect(&self) -> Chunk {
    //     Chunk::Connect {
    //         signal: self.signal_name.clone(),
    //         trampoline: self.trampoline_name.clone(),
    //         in_trait: self.in_trait,
    //     }
    // }
}
