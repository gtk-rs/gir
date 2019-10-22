use std::env;
use std::process;
use std::{cell::RefCell, str::FromStr};

use getopts::Options;
use hprof::Profiler;
use libgir::{self as gir, Config, Library, WorkMode};

fn print_usage(program: &str, opts: Options) {
    let brief = format!(
        "Usage: {program} [options] [<library> <version>]
       {program} (-h | --help)",
        program = program
    );
    print!("{}", opts.usage(&brief));
}

trait OptionStr {
    fn as_str_ref(&self) -> Option<&str>;
}

impl<S: AsRef<str>> OptionStr for Option<S> {
    fn as_str_ref(&self) -> Option<&str> {
        self.as_ref().map(|string| string.as_ref())
    }
}

fn build_config() -> Result<Config, String> {
    let args: Vec<_> = env::args().collect();
    let program = args[0].clone();

    let mut options = Options::new();
    options.optopt(
        "c",
        "config",
        "Config file path (default: Gir.toml)",
        "CONFIG",
    );
    options.optflag("h", "help", "Show this message");
    options.optopt("d", "gir-directory", "Directory for girs", "GIRSPATH");
    options.optopt(
        "m",
        "mode",
        "Work mode: doc, normal, sys or not_bound",
        "MODE",
    );
    options.optopt("o", "target", "Target path", "PATH");
    options.optopt("p", "doc-target-path", "Doc target path", "PATH");
    options.optflag("b", "make-backup", "Make backup before generating");
    options.optflag("s", "stats", "Show statistics");

    let matches = match options.parse(&args[1..]) {
        Ok(matches) => matches,
        Err(e) => return Err(e.to_string()),
    };

    if matches.opt_present("h") {
        print_usage(&program, options);
        process::exit(0);
    }

    let work_mode = match matches.opt_str("m") {
        None => None,
        Some(s) => match WorkMode::from_str(&s) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("Error (switching to default work mode): {}", e);
                None
            }
        },
    };

    Config::new(
        matches.opt_str("c").as_str_ref(),
        work_mode,
        matches.opt_str("d").as_str_ref(),
        matches.free.get(0).as_str_ref(),
        matches.free.get(1).as_str_ref(),
        matches.opt_str("o").as_str_ref(),
        matches.opt_str("doc-target-path").as_str_ref(),
        matches.opt_present("b"),
        matches.opt_present("s"),
    )
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    if let Err(ref e) = do_main() {
        eprintln!("{}", e);

        ::std::process::exit(1);
    }
}

fn do_main() -> Result<(), String> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "gir=warn");
        std::env::set_var("RUST_LOG", "libgir=warn");
    }
    env_logger::init();

    let mut cfg = match build_config() {
        Ok(cfg) => cfg,
        Err(err) => return Err(err),
    };

    let statistics = Profiler::new("Gir");
    statistics.start_frame();

    let watcher_total = statistics.enter("Total");

    let mut library;

    {
        let _watcher = statistics.enter("Loading");

        library = Library::new(&cfg.library_name);
        library.read_file(&cfg.girs_dir, &cfg.library_full_name())?;
    }

    {
        let _watcher = statistics.enter("Preprocessing");
        library.preprocessing(cfg.work_mode);
    }

    {
        let _watcher = statistics.enter("Update library by config");
        gir::update_version::apply_config(&mut library, &cfg);
    }

    {
        let _watcher = statistics.enter("Postprocessing");
        library.postprocessing(&cfg);
    }

    {
        let _watcher = statistics.enter("Resolving type ids");
        cfg.resolve_type_ids(&library);
    }

    {
        let _watcher = statistics.enter("Checking versions");
        gir::update_version::check_function_real_version(&mut library);
    }

    let mut env;

    {
        let _watcher = statistics.enter("Namespace/symbol/class analysis");

        let namespaces = gir::namespaces_run(&library);
        let symbols = gir::symbols_run(&library, &namespaces);
        let class_hierarchy = gir::class_hierarchy_run(&library);

        env = gir::Env {
            library,
            config: cfg,
            namespaces,
            symbols: RefCell::new(symbols),
            class_hierarchy,
            analysis: Default::default(),
        };
    }

    if env.config.work_mode != WorkMode::Sys {
        let _watcher = statistics.enter("Analysing");
        gir::analysis_run(&mut env);
    }

    if env.config.work_mode != WorkMode::DisplayNotBound {
        let _watcher = statistics.enter("Generating");
        gir::codegen_generate(&env);
    }

    drop(watcher_total);
    statistics.end_frame();

    if env.config.show_statistics {
        statistics.print_timing();
    }
    if env.config.work_mode == WorkMode::DisplayNotBound {
        env.library.show_non_bound_types(&env);
    }

    Ok(())
}
