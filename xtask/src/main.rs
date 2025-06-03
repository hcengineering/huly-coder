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
        Some("build-docker") => build_docker()?,
        Some("run-docker") => {
            let Some(data_dir) = env::args().nth(2) else {
                eprintln!("data_dir is required");
                std::process::exit(-1);
            };
            let Some(workspace_dir) = env::args().nth(3) else {
                eprintln!("workspace_dir is required");
                std::process::exit(-1);
            };
            run_docker(&data_dir, &workspace_dir)?;
        }
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

clean                                   cleans project directory from logs and artifacts
dist                                    builds application
build-docker                            builds docker image
run-docker <data_dir> <workspace_dir>   runs docker image
"
    )
}

fn clean() -> Result<(), DynError> {
    let _ = fs::remove_dir_all(project_root().join("data"));
    let _ = fs::remove_dir_all(project_root().join(".fastembed_cache"));
    let _ = fs::remove_dir_all(project_root().join("target/workspace"));
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

fn build_docker() -> Result<(), DynError> {
    let _ = Command::new("docker")
        .arg("build")
        .arg("-t")
        .arg("huly-coder")
        .arg("-f")
        .arg("./Dockerfile")
        .arg(".")
        .status()?;
    Ok(())
}

fn run_docker(data_dir: &str, workspace_dir: &str) -> Result<(), DynError> {
    let _ = Command::new("docker")
        .arg("run")
        .arg("-it")
        .arg("--rm")
        .arg("-e")
        .arg("DOCKER_RUN=1")
        .arg("-v")
        .arg(format!("{}:/target/workspace", workspace_dir))
        .arg("-v")
        .arg(format!("{}:/data", data_dir))
        .arg("-v")
        .arg(format!("{}/.fastembed_cache:/.fastembed_cache", data_dir))
        .arg("huly-coder")
        .status()?;

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
