use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("clean") => clean()?,
        Some("dist") => dist()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

clean            cleans project directory from logs and artifacts
dist             builds application
"
    )
}

fn clean() -> Result<(), DynError> {
    let _ = fs::remove_dir_all(project_root().join("logs"));
    let _ = fs::remove_dir_all(project_root().join(".fastembed_cache"));
    let _ = fs::remove_dir_all(project_root().join("target/workspace"));
    let _ = fs::remove_file(project_root().join("memory.yaml"));
    let _ = fs::remove_file(project_root().join("history.json"));
    let _ = fs::remove_file(project_root().join("openrouter_models.json"));
    Ok(())
}

fn dist() -> Result<(), DynError> {
    let _ = fs::remove_dir_all(dist_dir());
    fs::create_dir_all(dist_dir())?;

    dist_binary()?;

    Ok(())
}

fn dist_binary() -> Result<(), DynError> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(["build", "--release"])
        .status()?;

    if !status.success() {
        Err("cargo build failed")?;
    }

    // let dst = project_root().join("target/release/huly-coder");

    // fs::copy(&dst, dist_dir().join("hello-world"))?;

    // if Command::new("strip")
    //     .arg("--version")
    //     .stdout(Stdio::null())
    //     .status()
    //     .is_ok()
    // {
    //     eprintln!("stripping the binary");
    //     let status = Command::new("strip").arg(&dst).status()?;
    //     if !status.success() {
    //         Err("strip failed")?;
    //     }
    // } else {
    //     eprintln!("no `strip` utility found")
    // }

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn dist_dir() -> PathBuf {
    project_root().join("target/dist")
}
