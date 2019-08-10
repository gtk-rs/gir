use hprof::Profiler;
use libgir::{self as gir, Config, Library, WorkMode};
use std::{cell::RefCell, str::FromStr};

static USAGE: &str = "
Usage: gir [options] [<library> <version>]
       gir (-h | --help)

Options:
    -h, --help              Show this message.
    -c CONFIG               Config file path (default: Gir.toml)
    -d GIRSPATH             Directory for girs
    -m MODE                 Work mode: doc, normal, sys or not_bound
    -o PATH                 Target path
    --doc-target-path PATH  Doc target path
    -b, --make-backup       Make backup before generating
    -s, --stats             Show statistics
";

fn build_config() -> Result<Config, String> {
    let args = match docopt::Docopt::new(USAGE).and_then(|dopt| dopt.parse()) {
        Ok(args) => args,
        Err(e) => return Err(e.to_string()),
    };
    let work_mode = match args.get_str("-m") {
        "" => None,
        s => match WorkMode::from_str(s) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("Error (switching to default work mode): {}", e);
                None
            }
        },
    };

    Config::new(
        args.get_str("-c"),
        work_mode,
        args.get_str("-d"),
        args.get_str("<library>"),
        args.get_str("<version>"),
        args.get_str("-o"),
        args.get_str("--doc-target-path"),
        args.get_bool("-b"),
        args.get_bool("-s"),
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
