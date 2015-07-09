use env::Env;

mod ffi_type;
mod functions;
mod statics;

pub fn generate(env: &Env) {
    functions::generate(env);
}
