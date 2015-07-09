use env::Env;

mod ffi_type;
mod functions;
mod lib_;
mod statics;

pub fn generate(env: &Env) {
    lib_::generate(env);
}
