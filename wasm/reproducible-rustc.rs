use std::env;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mut input = env::args_os().skip(1);
    let compiler = input.next().expect("Cargo must supply the compiler");
    let arguments: Vec<_> = input.collect();
    let wasm = arguments
        .windows(2)
        .any(|pair| pair[0] == "--target" && pair[1] == "wasm32-unknown-unknown")
        || arguments
            .iter()
            .any(|arg| arg == "--target=wasm32-unknown-unknown");
    let mut command = Command::new(compiler);

    if wasm && env::var_os("CARGO_MANIFEST_DIR").is_some() {
        let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let source_root = PathBuf::from(env::var_os("SIEDIFF_SOURCE_ROOT").unwrap());
        let cargo_root = PathBuf::from(env::var_os("SIEDIFF_CARGO_ROOT").unwrap());
        let source = if let Ok(relative) = manifest.strip_prefix(source_root) {
            PathBuf::from("/siediff").join(relative)
        } else if let Ok(relative) = manifest.strip_prefix(cargo_root) {
            PathBuf::from("/cargo").join(relative)
        } else {
            panic!("Unknown Wasm package source");
        };
        let mut arguments = arguments.into_iter().peekable();

        while let Some(argument) = arguments.next() {
            if argument == "-C"
                && arguments
                    .peek()
                    .is_some_and(|next| next.to_string_lossy().starts_with("metadata="))
            {
                arguments.next();
            } else if !argument.to_string_lossy().starts_with("-Cmetadata=") {
                command.arg(argument);
            }
        }

        let name = env::var("CARGO_PKG_NAME").unwrap();
        let version = env::var("CARGO_PKG_VERSION").unwrap();
        command.args([
            OsString::from("-C"),
            OsString::from(format!(
                "metadata=siediff-wasm:{name}:{version}:{}",
                source.display()
            )),
        ]);
    } else {
        command.args(arguments);
    }

    panic!("Cannot execute compiler: {}", command.exec());
}
