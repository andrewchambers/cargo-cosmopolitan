"""Execute exactly the same APEs on each OS, checking both output and selected ISA."""
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

VARIANTS = ("debug", "release", "tiny", "optimized", "no-pthread", "c-tiny")


def main():
    directory = Path(sys.argv[1] if len(sys.argv) > 1 else "artifacts").resolve()
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}[machine]
    greeting = "CI with spaces"
    failures = []
    for name in VARIANTS:
        path = directory / f"{name}.com"
        if not path.is_file():
            raise RuntimeError(f"missing artifact: {path}")
        if os.name == "nt":
            command = [str(path)]
        else:
            path.chmod(path.stat().st_mode | 0o111)
            # Exercise the distributed APE bootstrap, including its embedded
            # loader. Runtime jobs do not install Cosmopolitan or rebuild Rust.
            command = ["/bin/sh", str(path)]
        result = subprocess.run(command + [greeting], capture_output=True, timeout=90)
        stdout = result.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n")
        stderr = result.stderr.decode("utf-8", errors="replace")
        print(f"--- {name} on {platform.system()}/{arch} (exit {result.returncode}) ---", flush=True)
        print(stdout, end="", flush=True)
        if stderr:
            print(stderr, file=sys.stderr, flush=True)
        if name == "c-tiny":
            if result.returncode != 0:
                failures.append(f"upstream C tiny pthread probe: exit {result.returncode}")
            continue
        expected = [f"hello from {greeting}", f"architecture: {arch}", f"pthread: {str(name != 'no-pthread').lower()}"]
        if result.returncode != 0 or any(line not in stdout.splitlines() for line in expected):
            failures.append(f"{name}: incorrect exit status, output, or selected architecture")
            continue
        message = "computed 42 without pthreads" if name == "no-pthread" else "a pthread computed 42"
        if f"{message} (through a pipe)" not in stdout or not re.search(r"^Unix time: [1-9][0-9]{9,}$", stdout, re.M):
            failures.append(f"{name}: missing pipe or clock result")
    if failures:
        raise RuntimeError("; ".join(failures))


if __name__ == "__main__":
    main()
