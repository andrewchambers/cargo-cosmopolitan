"""Build standalone demo variants. Requires cargo-cosmopolitan and cosmocc on PATH."""
import os
from pathlib import Path
import shutil
import subprocess

VARIANTS = {
    "debug": ([], {}),
    "release": (["--release"], {}),
    "optimized": (["--release"], {
        "CARGO_PROFILE_RELEASE_LTO": "true",
        "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1",
        "CARGO_PROFILE_RELEASE_STRIP": "symbols",
    }),
    "no-pthread": (["--release", "--no-default-features"], {}),
}


def main():
    output = Path("artifacts")
    output.mkdir(exist_ok=True)
    for name, (args, settings) in VARIANTS.items():
        env = dict(os.environ)
        for key in ("CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "CARGO_PROFILE_RELEASE_STRIP"):
            env.pop(key, None)
        env.update(settings)
        subprocess.run([
            "cargo", "cosmopolitan", "build", "--manifest-path", "demo/Cargo.toml",
            "--locked", *args,
        ], env=env, check=True)
        profile = "release" if "--release" in args else "debug"
        source = Path("demo/target/cosmopolitan") / profile / "cosmo-demo.com"
        shutil.copy2(source, output / f"{name}.com")
        print(f"{name}: {source.stat().st_size} bytes", flush=True)


if __name__ == "__main__":
    main()
