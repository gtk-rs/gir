#!/usr/bin/env python3

from pathlib import Path
import argparse
import subprocess
import sys

DEFAULT_GIR_FILES_DIRECTORY = Path("./gir-files")
DEFAULT_GIR_DIRECTORY = Path("./gir/")
DEFAULT_GIR_PATH = DEFAULT_GIR_DIRECTORY / "target/release/gir"


def run_command(command, folder=None):
    return subprocess.run(command, cwd=folder, check=True)


def spawn_process(command):
    return subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def update_workspace():
    return run_command(["cargo", "build", "--release"], "gir")


def ask_yes_no_question(question, conf):
    question = "{} [y/N] ".format(question)
    if conf.yes:
        print(question + "y")
        return True
    line = input(question)
    return line.strip().lower() == "y"


def update_submodule(submodule_path, conf):
    if any(submodule_path.iterdir()):
        return False
    print("=> Initializing {} submodule...".format(submodule_path))
    run_command(["git", "submodule", "update", "--init", submodule_path])
    print("<= Done!")

    if ask_yes_no_question(
        "Do you want to update {} submodule?".format(submodule_path), conf
    ):
        print("=> Updating submodule...")
        run_command(["git", "reset", "--hard", "HEAD"], submodule_path)
        run_command(["git", "pull", "-f", "origin", "master"], submodule_path)
        print("<= Done!")
        return True
    return False


def build_gir():
    print("=> Building gir...")
    update_workspace()
    print("<= Done!")


def regen_crates(path, conf):
    processes = []
    if path.is_dir():
        for entry in path.rglob("Gir*.toml"):
            processes += regen_crates(entry, conf)
    elif path.match("Gir*.toml"):
        args = [conf.gir_path, "-c", path, "-o", path.parent] + [
            d for path in conf.gir_files_paths for d in ("-d", path)
        ]

        is_sys_crate = path.parent.name.endswith("sys")

        if conf.docs:
            # Update docs/**/docs.md for non-sys crates
            if is_sys_crate:
                return processes

            doc_path = "docs.md"
            if isinstance(conf.docs, Path):
                # doc-target-path is relative to `-c`
                path_depth = len(path.parent.parts)
                doc_path = Path(*[".."] * path_depth, conf.docs, path.parent, doc_path)
            doc_args = args + ["-m", "doc", "--doc-target-path", doc_path]
            processes.append(
                (
                    "Regenerating documentation for `{}` into `{}`...".format(
                        path, doc_path
                    ),
                    spawn_process(doc_args),
                )
            )
        else:
            if is_sys_crate:
                args.extend(["-m", "sys"])
            processes.append(("Regenerating `{}`...".format(path), spawn_process(args)))

    else:
        raise Exception("`{}` is not a valid Gir*.toml file".format(path))

    return processes


def valid_path(path):
    path = Path(path)
    if not path.exists():
        raise argparse.ArgumentTypeError("`{}` no such file or directory".format(path))
    return path


def directory_path(path):
    path = Path(path)
    if not path.is_dir():
        raise argparse.ArgumentTypeError("`{}` directory not found".format(path))
    return path


def directory_output_path(path):
    """
    Creates an output directory if it doesn't exist yet.

    Fails if the directory cannot be created or the path exists but is not a directory.
    """
    path = Path(path)
    path.mkdir(parents=True, exist_ok=True)
    return path


def file_path(path):
    path = Path(path)
    if not path.is_file():
        raise argparse.ArgumentTypeError("`{}` file not found".format(path))
    return path


def parse_args():
    parser = argparse.ArgumentParser(
        description="Helper to regenerate gtk-rs crates using gir.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )

    parser.add_argument(
        "path",
        nargs="*",
        default=[Path(".")],
        type=valid_path,
        help="Paths in which to look for Gir.toml files",
    )
    parser.add_argument(
        "--gir-files-directories",
        nargs="+",  # If the option is used, we expect at least one folder!
        dest="gir_files_paths",
        default=[],
        type=directory_path,
        help="Path of the gir-files folder",
    )
    parser.add_argument(
        "--gir-path",
        default=DEFAULT_GIR_PATH,
        type=file_path,
        help="Path of the gir executable to run",
    )
    parser.add_argument(
        "--yes",
        action="store_true",
        help=" Always answer `yes` to any question asked by the script",
    )
    parser.add_argument(
        "--no-fmt",
        action="store_true",
        help="If set, this script will not run `cargo fmt`",
    )
    parser.add_argument(
        "--docs",
        metavar="output_path",
        nargs="?",
        const=True,
        default=False,
        type=directory_output_path,
        help="Build documentation with `gir -m doc`. Optionally takes an output directory",
    )

    return parser.parse_args()


def main():
    conf = parse_args()

    if not conf.gir_files_paths:
        update_submodule(DEFAULT_GIR_FILES_DIRECTORY, conf)

    if conf.gir_path == DEFAULT_GIR_PATH:
        update_submodule(DEFAULT_GIR_DIRECTORY, conf)
        build_gir()

    print("=> Regenerating crates...")
    for path in conf.path:
        print("=> Looking in path `{}`".format(path))
        processes = regen_crates(path, conf)
        for log, p in processes:
            print("==> {}".format(log))
            stdout, stderr = p.communicate()
            stdout = stdout.decode("utf-8")
            stderr = stderr.decode("utf-8")
            assert p.returncode == 0, stderr.strip()
            # Gir doesn't print anything to stdout. If it does, this is likely out of
            # order with stderr, unless the printer/logging flushes in between.
            assert not stdout, "`gir` printed unexpected stdout: {}".format(stdout)
            print(stderr, end="")

    if not conf.no_fmt and not run_command(["cargo", "fmt"]):
        return 1
    print("<= Done!")
    print("Don't forget to check if everything has been correctly generated!")
    return 0


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print("Error: {}".format(e), file=sys.stderr)
        sys.exit(1)
