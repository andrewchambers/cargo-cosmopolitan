use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ARCHES: [&str; 2] = ["x86_64", "aarch64"];
const HELP: &str = "cargo cosmopolitan <preflight|build|run> [options]

Build no_std + alloc applications as x86-64 + ARM64 Actually Portable Executables.
Requires Cosmopolitan on PATH, and a Rust nightly with rust-src.

Options:
  --toolchain NAME          Rust toolchain (default: nightly-2026-01-21)
  --tiny                    Use Cosmopolitan's size-optimized runtime
  --release                 Build the release profile
  --manifest-path PATH      Application Cargo.toml
  -p, --package NAME         Workspace package
  --bin NAME                Binary target
  --example NAME            Example target
  --features FEATURES       Cargo feature selection
  --all-features, --no-default-features, --locked, --offline, -v
  -- ARGS                   Arguments to the program (run only)

Example:
  cargo cosmopolitan run --release --manifest-path demo/Cargo.toml -- World

Toolchains are discovered locally; this tool never downloads or installs them.";

struct Options {
    action: String,
    toolchain: OsString,
    cargo: Vec<OsString>,
    program: Vec<OsString>,
    release: bool,
    tiny: bool,
}

fn parse(mut args: Vec<OsString>) -> Result<Option<Options>> {
    if args.first().is_some_and(|a| a == "cosmopolitan") {
        args.remove(0);
    }
    if args.is_empty()
        || args.iter().any(|a| a == "--help" || a == "-h") && !args.iter().any(|a| a == "--")
    {
        println!("{HELP}");
        return Ok(None);
    }
    let action = args
        .remove(0)
        .into_string()
        .map_err(|_| "invalid command")?;
    if !matches!(action.as_str(), "preflight" | "build" | "run") {
        return Err(format!("unknown command {action:?}; use --help").into());
    }
    let mut opts = Options {
        action,
        toolchain: "nightly-2026-01-21".into(),
        cargo: Vec::new(),
        program: Vec::new(),
        release: false,
        tiny: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") if opts.action == "run" => {
                opts.program.extend(args);
                break;
            }
            Some("--toolchain") => {
                opts.toolchain = args.next().ok_or("--toolchain needs a value")?
            }
            Some("--tiny") => opts.tiny = true,
            Some("--release") => {
                opts.release = true;
                opts.cargo.push(arg);
            }
            Some("--manifest-path" | "--package" | "-p" | "--bin" | "--example" | "--features") => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{} needs a value", arg.to_string_lossy()))?;
                opts.cargo.extend([arg, value]);
            }
            Some(
                "--all-features"
                | "--no-default-features"
                | "--locked"
                | "--offline"
                | "-v"
                | "--verbose",
            ) => opts.cargo.push(arg),
            _ => {
                return Err(
                    format!("unsupported option {}; use --help", arg.to_string_lossy()).into(),
                );
            }
        }
    }
    Ok(Some(opts))
}

fn checked(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!(
            "{} failed ({status})",
            command.get_program().to_string_lossy()
        )
        .into());
    }
    Ok(())
}

fn output(command: &mut Command) -> Result<Vec<u8>> {
    let result = command.stderr(Stdio::inherit()).output()?;
    if !result.status.success() {
        return Err(format!(
            "{} failed ({})",
            command.get_program().to_string_lossy(),
            result.status
        )
        .into());
    }
    Ok(result.stdout)
}

fn find(name: &str) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join(name);
        if path.is_file() && fs::metadata(&path)?.permissions().mode() & 0o111 != 0 {
            // Preserve symlink names: Cosmopolitan dispatches on argv[0].
            return Ok(if path.is_absolute() {
                path
            } else {
                env::current_dir()?.join(path)
            });
        }
    }
    Err(format!("{name} was not found on PATH; add the Cosmopolitan bin directory to PATH").into())
}

fn magic(path: &Path) -> Result<[u8; 4]> {
    let mut bytes = [0; 4];
    fs::File::open(path)?.read_exact(&mut bytes)?;
    Ok(bytes)
}

// Some kernels do not execute APE headers directly. Shells implement the
// ENOEXEC fallback; Command does not. Use the header's shell bootstrap for tools.
fn tool(path: &Path) -> Result<Command> {
    let bytes = magic(path)?;
    if &bytes == b"MZqF" || &bytes == b"jart" {
        let mut command = Command::new("/bin/sh");
        command.arg(path);
        Ok(command)
    } else {
        Ok(Command::new(path))
    }
}

fn launcher(path: &Path, program: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let command = tool(program)?;
    let quote = |s: &OsStr| -> Result<String> {
        let s = s.to_str().ok_or("tool paths must be UTF-8")?;
        Ok(format!("'{}'", s.replace('\'', "'\\''")))
    };
    let mut script = format!("#!/bin/sh\nexec {}", quote(command.get_program())?);
    for arg in command.get_args() {
        script.push(' ');
        script.push_str(&quote(arg)?);
    }
    script.push_str(" \"$@\"\n");
    if fs::read(path).ok().as_deref() != Some(script.as_bytes()) {
        fs::write(path, script)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

struct Toolchain {
    bin: PathBuf,
    rustup: PathBuf,
}
impl Toolchain {
    fn rust(&self, opts: &Options, program: &str) -> Command {
        let mut cmd = Command::new(&self.rustup);
        cmd.args([OsStr::new("run"), &opts.toolchain, OsStr::new(program)]);
        cmd
    }
    fn path(&self, name: &str) -> PathBuf {
        self.bin.join(name)
    }
    fn cc(&self, arch: &str) -> PathBuf {
        self.path(&format!("{arch}-unknown-cosmo-cc"))
    }
    fn preflight(opts: &Options) -> Result<Self> {
        let cosmocc = find("cosmocc")?;
        let bin = cosmocc
            .parent()
            .ok_or("cosmocc has no parent directory")?
            .to_path_buf();
        let tc = Self {
            bin,
            rustup: find("rustup")?,
        };
        for name in [
            "apelink",
            "pecheck",
            "ape-x86_64.elf",
            "ape-aarch64.elf",
            "ape-m1.c",
        ] {
            if !tc.path(name).is_file() {
                return Err(format!(
                    "incomplete Cosmopolitan installation: missing {}",
                    tc.path(name).display()
                )
                .into());
            }
        }
        for arch in ARCHES {
            for name in [
                format!("{arch}-unknown-cosmo-cc"),
                format!("{arch}-linux-cosmo-ar"),
            ] {
                if !tc.path(&name).is_file() {
                    return Err(format!("missing {}", tc.path(&name).display()).into());
                }
            }
        }
        let version = String::from_utf8(output(tc.rust(opts, "rustc").arg("--version"))?)?;
        if !version.contains("nightly") {
            return Err("a nightly Rust toolchain is required (--toolchain nightly)".into());
        }
        let sysroot =
            String::from_utf8(output(tc.rust(opts, "rustc").args(["--print", "sysroot"]))?)?;
        if !Path::new(sysroot.trim())
            .join("lib/rustlib/src/rust/library/core/Cargo.toml")
            .is_file()
        {
            return Err(format!(
                "rust-src is missing; run: rustup component add rust-src --toolchain {}",
                opts.toolchain.to_string_lossy()
            )
            .into());
        }
        eprintln!("cargo-cosmopolitan: {}", version.trim());
        eprintln!("cargo-cosmopolitan: Cosmopolitan at {}", tc.bin.display());
        let temp = TempDir::new()?;
        let source = temp.0.join("probe.c");
        fs::write(
            &source,
            "#include <pthread.h>\n#ifndef __COSMOPOLITAN__\n#error expected Cosmopolitan\n#endif\nint main(void) { return !pthread_equal(pthread_self(), pthread_self()); }\n",
        )?;
        for arch in ARCHES {
            let object = temp.0.join(format!("{arch}.o"));
            let executable = temp.0.join(format!("{arch}.elf"));
            let mut cc = tool(&tc.cc(arch))?;
            cc.arg("-std=c99")
                .arg("-c")
                .arg(&source)
                .arg("-o")
                .arg(&object);
            if opts.tiny {
                cc.arg("-mtiny");
            }
            checked(&mut cc).map_err(|e| format!("{arch} C compiler preflight failed: {e}"))?;
            checked(
                tool(&tc.path(&format!("{arch}-linux-cosmo-ar")))?
                    .arg("crs")
                    .arg(temp.0.join(format!("{arch}.a")))
                    .arg(&object),
            )?;
            let mut cc = tool(&tc.cc(arch))?;
            cc.arg(&object).arg("-o").arg(&executable);
            if opts.tiny {
                cc.arg("-mtiny");
            }
            checked(&mut cc).map_err(|e| format!("{arch} linker preflight failed: {e}"))?;
            if magic(&executable)? != *b"\x7fELF" {
                return Err(format!("{arch} compiler did not produce an ELF executable").into());
            }
        }
        let rust_source = temp.0.join("host.rs");
        fs::write(&rust_source, "fn main() {}\n")?;
        checked(
            tc.rust(opts, "rustc")
                .arg(&rust_source)
                .arg("-o")
                .arg(temp.0.join("host")),
        )
        .map_err(|e| format!("host Rust compiler/linker preflight failed: {e}"))?;
        eprintln!("cargo-cosmopolitan: preflight passed (host Rust and both C toolchains)");
        Ok(tc)
    }
}

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Result<Self> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path =
            env::temp_dir().join(format!("cargo-cosmopolitan-{}-{stamp}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn target(tc: &Toolchain, opts: &Options, arch: &str, directory: &Path) -> Result<PathBuf> {
    let base = format!("{arch}-unknown-linux-gnu");
    let mut spec: Value = serde_json::from_slice(&output(tc.rust(opts, "rustc").args([
        "-Zunstable-options",
        "--print",
        "target-spec-json",
        "--target",
        &base,
    ]))?)?;
    let obj = spec
        .as_object_mut()
        .ok_or("invalid Rust target specification")?;
    // Keep LLVM's architecture layout, but give Rust dependencies an explicit
    // target identity and replace all platform linking defaults.
    for key in [
        "is-builtin",
        "metadata",
        "pre-link-args",
        "late-link-args",
        "late-link-args-static",
        "late-link-args-dynamic",
        "post-link-args",
        "pre-link-objects",
        "post-link-objects",
        "pre-link-objects-fallback",
        "post-link-objects-fallback",
        "link-self-contained",
        "supported-sanitizers",
        "supports-xray",
    ] {
        obj.remove(key);
    }
    for (key, value) in json!({
        "os": "cosmopolitan", "env": "cosmo", "vendor": "unknown",
        "target-family": ["unix"], "executables": true,
        "linker": env::current_exe()?, "linker-flavor": "gnu-cc",
        "dynamic-linking": false, "has-rpath": false,
        "position-independent-executables": false,
        "static-position-independent-executables": false,
        "relocation-model": "static", "relro-level": "off",
        "crt-static-default": true, "crt-static-respected": true,
        "has-thread-local": false, "disable-redzone": true,
        "panic-strategy": "abort", "frame-pointer": "always",
        "default-uwtable": false
    })
    .as_object()
    .unwrap()
    {
        obj.insert(key.clone(), value.clone());
    }
    if arch == "aarch64" {
        obj.insert(
            "features".into(),
            json!("+v8a,+outline-atomics,+reserve-x18,+reserve-x28"),
        );
    }
    // Make runtime selection part of the target specification and Cargo's
    // fingerprint, so changing --tiny always causes a relink.
    if opts.tiny {
        obj.insert("pre-link-args".into(), json!({"gnu-cc": ["-mtiny"]}));
    }
    let path = directory.join(format!("{arch}-unknown-cosmopolitan.json"));
    let bytes = serde_json::to_vec_pretty(&spec)?;
    if fs::read(&path).ok().as_deref() != Some(&bytes) {
        fs::write(&path, bytes)?;
    }
    Ok(path)
}

// Called by rustc using this executable as the target linker. Environment is
// scoped to the target Cargo build; host build scripts use their normal linker.
fn link() -> Result<()> {
    let cc = env::var_os("CARGO_COSMO_LINK_CC").ok_or("missing Cosmopolitan linker compiler")?;
    let mut command = tool(Path::new(&cc))?;
    for arg in env::args_os().skip(1) {
        match arg.to_str() {
            // fixupobj needs the ELF symbol table. apelink produces the final
            // distribution image; strip=true must not strip its input first.
            Some("-Wl,--strip-all" | "-Wl,-s" | "-s") => {}
            Some(
                "-lc" | "-lpthread" | "-lm" | "-lrt" | "-ldl" | "-lgcc" | "-lgcc_s" | "-lunwind"
                | "-lutil" | "-nodefaultlibs" | "-nostartfiles" | "-nostdlib",
            ) => {}
            Some("-pie" | "-shared") => {
                return Err("PIE and shared libraries are unsupported by this APE target".into());
            }
            _ => {
                command.arg(arg);
            }
        }
    }
    checked(&mut command)
}

fn build(tc: &Toolchain, opts: &Options) -> Result<Vec<PathBuf>> {
    let mut metadata = tc.rust(opts, "cargo");
    metadata.args(["metadata", "--format-version=1", "--no-deps"]);
    if let Some(index) = opts.cargo.iter().position(|a| a == "--manifest-path") {
        metadata.args(&opts.cargo[index..=index + 1]);
    }
    let data: Value = serde_json::from_slice(&output(&mut metadata)?)?;
    let root = PathBuf::from(
        data["target_directory"]
            .as_str()
            .ok_or("Cargo did not report a target directory")?,
    )
    .join("cosmopolitan");
    let specs = root.join("targets");
    fs::create_dir_all(&specs)?;
    let mut artifacts = Vec::new();
    for arch in ARCHES {
        let spec = target(tc, opts, arch, &specs)?;
        // cc-rs executes AR directly, but the archiver may itself be an APE.
        let ar = specs.join(format!("{arch}-ar"));
        launcher(&ar, &tc.path(&format!("{arch}-linux-cosmo-ar")))?;
        let target_name = spec
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace('-', "_");
        let mut cmd = tc.rust(opts, "cargo");
        cmd.args([
            "build",
            "-Zbuild-std=core,alloc",
            "-Zbuild-std-features=",
            "--message-format=json-render-diagnostics",
        ])
        .arg("--target")
        .arg(&spec)
        .arg("--target-dir")
        .arg(&root)
        .args([
            "--config",
            "profile.dev.panic=\"abort\"",
            "--config",
            "profile.release.panic=\"abort\"",
        ])
        .args(&opts.cargo)
        .env(format!("CC_{target_name}"), tc.cc(arch))
        .env(format!("AR_{target_name}"), &ar)
        .env("CARGO_COSMO_LINK_CC", tc.cc(arch))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
        if opts.tiny {
            cmd.env(format!("CFLAGS_{target_name}"), "-mtiny");
        }
        eprintln!("cargo-cosmopolitan: building {arch}");
        let mut child = cmd.spawn()?;
        let mut found = BTreeMap::new();
        for line in BufReader::new(child.stdout.take().unwrap()).lines() {
            let line = line?;
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if value["reason"] == "compiler-artifact" {
                    if let Some(executable) = value["executable"].as_str() {
                        let kind = value["target"]["kind"][0].as_str().unwrap_or("");
                        if kind == "bin" || kind == "example" {
                            let name = value["target"]["name"]
                                .as_str()
                                .ok_or("artifact has no name")?;
                            found.insert(
                                (
                                    value["package_id"].to_string(),
                                    kind.to_owned(),
                                    name.to_owned(),
                                ),
                                PathBuf::from(executable),
                            );
                        }
                    }
                } else if value["reason"] == "compiler-message" {
                    if let Some(rendered) = value["message"]["rendered"].as_str() {
                        eprint!("{rendered}");
                    }
                }
            } else {
                eprintln!("{line}");
            }
        }
        if !child.wait()?.success() {
            return Err(format!("{arch} Cargo build failed").into());
        }
        if found.is_empty() {
            return Err("no executable selected; use --bin NAME or --example NAME".into());
        }
        for path in found.values() {
            if magic(path)? != *b"\x7fELF" {
                return Err(format!("expected an ELF executable at {}", path.display()).into());
            }
        }
        artifacts.push(found);
    }
    if artifacts[0].keys().ne(artifacts[1].keys()) {
        return Err("the two architecture builds produced different executable targets".into());
    }
    let outdir = root.join(if opts.release { "release" } else { "debug" });
    fs::create_dir_all(&outdir)?;
    let mut result = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for (key, x86) in &artifacts[0] {
        let name = &key.2;
        if !names.insert(name) {
            return Err(
                format!("multiple executables named {name}; select one package/target").into(),
            );
        }
        let destination = outdir.join(format!("{name}.com"));
        let temporary = outdir.join(format!(".{name}-{}.tmp", std::process::id()));
        let mut cmd = tool(&tc.path("apelink"))?;
        cmd.args(["-V", "-1"])
            .arg("-l")
            .arg(tc.path("ape-x86_64.elf"))
            .arg("-l")
            .arg(tc.path("ape-aarch64.elf"))
            .arg("-M")
            .arg(tc.path("ape-m1.c"))
            .arg("-o")
            .arg(&temporary)
            .arg(x86)
            .arg(&artifacts[1][key]);
        let packaging = (|| -> Result<()> {
            checked(&mut cmd)?;
            checked(tool(&tc.path("pecheck"))?.arg(&temporary))?;
            fs::rename(&temporary, &destination)?;
            Ok(())
        })();
        if packaging.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        packaging?;
        eprintln!(
            "cargo-cosmopolitan: {} ({} bytes, x86_64 + aarch64)",
            destination.display(),
            fs::metadata(&destination)?.len()
        );
        result.push(destination);
    }
    Ok(result)
}

fn run() -> Result<i32> {
    if env::var_os("CARGO_COSMO_LINK_CC").is_some() {
        link()?;
        return Ok(0);
    }
    let Some(opts) = parse(env::args_os().skip(1).collect())? else {
        return Ok(0);
    };
    let tc = Toolchain::preflight(&opts)?;
    if opts.action == "preflight" {
        return Ok(0);
    }
    let artifacts = build(&tc, &opts)?;
    if opts.action == "run" {
        if artifacts.len() != 1 {
            return Err("run requires exactly one executable; use --bin or --example".into());
        }
        // On Linux, use the native loader explicitly, including on WSL where
        // invoking an APE path can otherwise launch the Windows half.
        let mut cmd = if cfg!(target_os = "linux") {
            let mut cmd = Command::new(tc.path(&format!("ape-{}.elf", env::consts::ARCH)));
            cmd.arg(&artifacts[0]);
            cmd
        } else {
            tool(&artifacts[0])?
        };
        let status = cmd.args(&opts.program).status()?;
        return Ok(status.code().unwrap_or(1));
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("cargo-cosmopolitan: {error}");
            std::process::exit(1);
        }
    }
}
