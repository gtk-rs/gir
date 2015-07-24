use env::Env;

mod build;
mod ffi_type;
mod functions;
mod lib_;
mod statics;

pub fn generate(env: &Env) {
    lib_::generate(env);
    build::generate(env);
}
