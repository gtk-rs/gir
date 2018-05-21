use analysis::conversion_type::ConversionType;
use analysis::functions::{AsyncTrampoline, find_index_to_ignore};
use analysis::function_parameters::CParameter as AnalysisCParameter;
use analysis::function_parameters::{Transformation, TransformationType};
use analysis::out_parameters::Mode;
use analysis::namespaces;
use analysis::return_value;
use analysis::rust_type::rust_type;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use chunk::{Chunk, Param, TupleMode};
use chunk::parameter_ffi_call_out;
use env::Env;
use library::{self, ParameterDirection};
use nameutil;


#[derive(Default)]
pub struct BaseBuilder {
    method_name: String,
    trampoline_name: String,
    in_trait: bool,
    function_type_string: String,
}

impl BaseBuilder {
    pub fn new() -> BaseBuilder {
        Default::default()
    }

    // pub fn method_name(&mut self, name: &str) -> &mut BaseBuilder {
    //     self.signal_name = name.into();
    //     self
    // }

    pub fn trampoline_name(&mut self, name: &str) -> &mut BaseBuilder {
        self.trampoline_name = name.into();
        self
    }

    pub fn in_trait(&mut self, value: bool) -> &mut BaseBuilder {
        self.in_trait = value;
        self
    }

    pub fn function_type_string(&mut self, type_: &str) -> &mut BaseBuilder {
        self.function_type_string = type_.into();
        self
    }

    pub fn generate(&self, env: &Env) -> Chunk {
        let mut body = Vec::new();


        // body.push(self.let_func());
        // body.push(self.connect());

        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();
        chunks.push(unsafe_);
        Chunk::BlockHalf(chunks)
    }

    // fn let_func(&self) -> Chunk {
    //     let type_ = format!("Box_<Box_<{}>>", self.function_type_string);
    //     Chunk::Let {
    //         name: "f".to_owned(),
    //         is_mut: false,
    //         value: Box::new(Chunk::Custom("Box_::new(Box_::new(f))".to_owned())),
    //         type_: Some(Box::new(Chunk::Custom(type_))),
    //     }
    // }

    // fn connect(&self) -> Chunk {
    //     Chunk::Connect {
    //         signal: self.signal_name.clone(),
    //         trampoline: self.trampoline_name.clone(),
    //         in_trait: self.in_trait,
    //     }
    // }
}
