#!/usr/bin/env python3

from pathlib import Path
import argparse
import subprocess
import sys
import asyncio

DEFAULT_GIR_FILES_DIRECTORY = Path("./gir-files")
DEFAULT_GIR_DIRECTORY = Path("./gir/")
DEFAULT_GIR_PATH = DEFAULT_GIR_DIRECTORY / "target/release/gir"


def run_command(command, folder=None):
    return subprocess.run(command, cwd=folder, check=True)


async def spawn_process(exe, args):
    p = await asyncio.create_subprocess_exec(
        str(exe),
        *(str(arg) for arg in args),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )

    stdout, stderr = await p.communicate()
    stdout = stdout.decode("utf-8")
    stderr = stderr.decode("utf-8")
    assert p.returncode == 0, stderr.strip()
    return stdout, stderr


async def spawn_gir(gir_exe, args):
    stdout, stderr = await spawn_process(gir_exe, args)
    # Gir doesn't print anything to stdout. If it does, this is likely out of
    # order with stderr, unless the printer/logging flushes in between.
    assert not stdout, "`gir` printed unexpected stdout: {}".format(stdout)
    if stderr:
        return "===> stderr:\n\n" + stderr + "\n"
    return ""


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


async def regenerate_crate_docs(conf, crate_dir, base_gir_args):
    doc_path = "docs.md"
    # Generate into docs.md instead of the default vendor.md
    doc_args = base_gir_args + ["-m", "doc", "--doc-target-path", doc_path]

    # The above `gir -m doc` generates docs.md relative to the directory containing Gir.toml
    doc_path = crate_dir / doc_path
    embed_args = ["-m", "-d", crate_dir / "src"]

    logs = ""

    if conf.strip_docs:
        logs += "==> Stripping documentation from `{}`...\n".format(crate_dir)
        # -n dumps stripped docs to stdout
        _, stderr = await spawn_process("rustdoc-stripper", embed_args + ["-s", "-n"])
        if stderr:
            logs += "===> stderr:\n\n" + stderr + "\n"

    if conf.embed_docs:
        logs += "==> Regenerating documentation for `{}` into `{}`...\n".format(
            crate_dir, doc_path
        )
        logs += await spawn_gir(conf.gir_path, doc_args)

        logs += "==> Embedding documentation from `{}` into `{}`...\n".format(
            doc_path, crate_dir
        )
        stdout, stderr = await spawn_process(
            "rustdoc-stripper", embed_args + ["-g", "-o", doc_path]
        )
        if stdout:
            logs += "===> stdout:\n\n" + stdout + "\n"
        if stderr:
            logs += "===> stderr:\n\n" + stderr + "\n"

    return logs


def regen_crates(path, conf):
    processes = []
    if path.is_dir():
        for entry in path.rglob("Gir*.toml"):
            processes += regen_crates(entry, conf)
    elif path.match("Gir*.toml"):
        args = ["-c", path, "-o", path.parent] + [
            d for path in conf.gir_files_paths for d in ("-d", path)
        ]

        is_sys_crate = path.parent.name.endswith("sys")

        if conf.embed_docs or conf.strip_docs:
            # Embedding documentation only applies to non-sys crates
            if is_sys_crate:
                return processes

            processes.append(regenerate_crate_docs(conf, path.parent, args))
        else:
            if is_sys_crate:
                args.extend(["-m", "sys"])

            async def regenerate_crate(path, args):
                return "==> Regenerating `{}`...\n".format(path) + await spawn_gir(
                    conf.gir_path, args
                )

            processes.append(regenerate_crate(path, args))

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
        "--embed-docs",
        action="store_true",
        help="Build documentation with `gir -m doc`, and embed it with `rustdoc-stripper -g`",
    )
    parser.add_argument(
        "--strip-docs",
        action="store_true",
        help="Remove documentation with `rustdoc-stripper -s -n`. Can be used in conjunction with --embed-docs",
    )

    return parser.parse_args()


async def main():
    conf = parse_args()

    if not conf.gir_files_paths:
        update_submodule(DEFAULT_GIR_FILES_DIRECTORY, conf)

    if conf.gir_path == DEFAULT_GIR_PATH:
        update_submodule(DEFAULT_GIR_DIRECTORY, conf)
        build_gir()

    print("=> Regenerating crates...")
    for path in conf.path:
        print("=> Looking in path `{}`".format(path))
        # Collect and print the results as soon as they trickle in, one process at a time:
        for coro in asyncio.as_completed(regen_crates(path, conf)):
            print(await coro, end="")

    if not conf.no_fmt and not run_command(["cargo", "fmt"]):
        return 1
    print("<= Done!")
    print("Don't forget to check if everything has been correctly generated!")
    return 0


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:
        print("Error: {}".format(e), file=sys.stderr)
        sys.exit(1)
