use argh::FromArgs;
use bootloader::disk_image::create_disk_image;
use std::{
    fs,
    io::{self, Error, ErrorKind},
    path::{Path, PathBuf},
    process::Command,
};

type ExitCode = i32;

#[derive(FromArgs)]
/// Build the bootloader
struct BuildArguments {
    /// path to the `Cargo.toml` of the kernel
    #[argh(option, default = "PathBuf::from(\"../kernel/Cargo.toml\")")]
    kernel_manifest: PathBuf,

    /// path to the kernel ELF binary
    #[argh(option, default = "PathBuf::from(\"../target/target/debug/kernel\")")]
    kernel_binary: PathBuf,

    /// whether to run the resulting binary in QEMU
    #[argh(switch)]
    run: bool,

    /// whether to run the resulting binary in QEMU
    #[argh(switch)]
    gdb: bool,

    /// whether to run the resulting binary in QEMU
    #[argh(switch)]
    dump: bool,

    /// suppress stdout output
    #[argh(switch)]
    quiet: bool,

    /// build the bootloader with the given cargo features
    #[argh(option)]
    features: Vec<String>,

    /// use the given path as target directory
    #[argh(option)]
    target_dir: Option<PathBuf>,

    /// place the output binaries at the given path
    #[argh(option)]
    out_dir: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let args: BuildArguments = argh::from_env();

    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir("kernel");
    cmd.args(&["build"]);
    assert!(cmd.status()?.success());

    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir("bootloader");
    cmd.arg("build").arg("--bin").arg("bios");
    cmd.arg("--profile").arg("release");
    cmd.arg("-Z").arg("unstable-options");
    cmd.arg("--target").arg("x86_64-bootloader.json");
    cmd.arg("--features")
        .arg(args.features.join(" ") + " bios_bin");
    cmd.arg("-Zbuild-std=core");
    cmd.arg("-Zbuild-std-features=compiler-builtins-mem");
    if let Some(target_dir) = &args.target_dir {
        cmd.arg("--target-dir").arg(target_dir);
    }
    if args.quiet {
        cmd.arg("--quiet");
    }
    cmd.env("KERNEL", &args.kernel_binary);
    cmd.env("KERNEL_MANIFEST", &args.kernel_manifest);
    cmd.env("RUSTFLAGS", "-C opt-level=s");
    assert!(cmd.status()?.success());

    // Retrieve binary paths
    cmd.arg("--message-format").arg("json");
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let mut executables = Vec::new();
    for line in String::from_utf8(output.stdout)
        .map_err(|err| Error::new(ErrorKind::Other, err))?
        .lines()
    {
        let mut artifact = json::parse(line).map_err(|err| Error::new(ErrorKind::Other, err))?;
        if let Some(executable) = artifact["executable"].take_string() {
            executables.push(PathBuf::from(executable));
        }
    }

    assert_eq!(executables.len(), 1);
    let executable_path = executables.pop().unwrap();
    let executable_name = executable_path.file_name().unwrap().to_str().unwrap();
    let kernel_name = args.kernel_binary.file_name().unwrap().to_str().unwrap();
    let mut output_bin_path = executable_path
        .parent()
        .unwrap()
        .join(format!("boot-{}-{}.img", executable_name, kernel_name));

    create_disk_image(&executable_path, &output_bin_path)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to create bootable diskimage"))?;

    if let Some(out_dir) = &args.out_dir {
        let file = out_dir.join(output_bin_path.file_name().unwrap());
        fs::copy(output_bin_path, &file)?;
        output_bin_path = file;
    }

    if !args.quiet {
        println!(
            "Created bootable disk image at {}",
            output_bin_path.display()
        );
    }

    if args.run {
        bios_run(&output_bin_path, args.gdb, args.dump)?;
    }

    Ok(())
}

fn bios_run(bin_path: &Path, gdb: bool, dump: bool) -> io::Result<Option<ExitCode>> {
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive")
        .arg(format!("format=raw,file={}", bin_path.display()))
        .args(&["-serial", "stdio"]);

    if gdb {
        qemu.args(&["-s", "-S"]);
    }
    if dump {
        qemu.args(&["-d", "int,cpu_reset", "-no-reboot"]);
    }

    println!("{:?}", qemu);
    let exit_status = qemu.status()?;
    let ret = if exit_status.success() {
        None
    } else {
        exit_status.code()
    };
    Ok(ret)
}
