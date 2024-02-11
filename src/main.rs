use std::{
    borrow::Borrow,
    fs::File,
    io::{Read, Write},
};

use indicatif::{ProgressBar, ProgressStyle};

use flate2;
use reqwest;
use tar;
use tokio;

use core::time::Duration;

static LIST_ARCHS: &[&str] = &[
    "i386",
    "i586",
    "i686",
    "x86_64",
    "arm",
    "armv7",
    "armv7s",
    "aarch64",
    "mips",
    "mipsel",
    "mips64",
    "mips64el",
    "powerpc",
    "powerpc64",
    "powerpc64le",
    "riscv64gc",
    "s390x",
    "loongarch64",
];
static LIST_OSES: &[&str] = &[
    "pc-windows",
    "unknown-linux",
    "apple-darwin",
    "unknown-netbsd",
    "apple-ios",
    "linux",
    "rumprun-netbsd",
    "unknown-freebsd",
    "unknown-illumos",
];
static LIST_ENVS: &[&str] = &[
    "gnu",
    "gnux32",
    "msvc",
    "gnueabi",
    "gnueabihf",
    "gnuabi64",
    "androideabi",
    "android",
    "musl",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetTriple {
    pub arch: Option<String>,
    pub os: Option<String>,
    pub env: Option<String>,
}

impl TargetTriple {
    pub fn str(&self) -> String {
        let mut triple = String::new();
        if let Some(arch) = &self.arch {
            triple.push_str(arch);
        }
        if let Some(os) = &self.os {
            triple.push_str("-");
            triple.push_str(os);
        }
        if let Some(env) = &self.env {
            triple.push_str("-");
            triple.push_str(env);
        }
        triple
    }

    pub fn new(arch: Option<String>, os: Option<String>, env: Option<String>) -> Self {
        TargetTriple { arch, os, env }
    }

    pub fn from_target_triple(triple: &str) -> Self {
        let mut parts = triple.split('-');
        let arch = parts.next().map(|s| s.to_string());
        let os = parts.next().map(|s| s.to_string());
        let env = parts.next().map(|s| s.to_string());
        TargetTriple { arch, os, env }
    }

    pub fn to_target_triple(&self) -> String {
        let mut triple = String::new();
        if let Some(arch) = &self.arch {
            triple.push_str(arch);
        }
        if let Some(os) = &self.os {
            triple.push_str("-");
            triple.push_str(os);
        }
        if let Some(env) = &self.env {
            triple.push_str("-");
            triple.push_str(env);
        }
        triple
    }

    pub fn is_valid(&self) -> bool {
        if let Some(arch) = &self.arch {
            if !LIST_ARCHS.contains(&arch.as_str()) {
                return false;
            }
        }
        if let Some(os) = &self.os {
            if !LIST_OSES.contains(&os.as_str()) {
                return false;
            }
        }
        if let Some(env) = &self.env {
            if !LIST_ENVS.contains(&env.as_str()) {
                return false;
            }
        }
        true
    }

    pub fn get_with_no_rust_installed() -> Self {
        let arch = std::env::consts::ARCH.to_string();
        let os = std::env::consts::OS.to_string();

        let os_matching = match os.as_str() {
            "linux" => "unknown-linux",
            "macos" => "apple-darwin",
            "windows" => "pc-windows",
            "netbsd" => {
                if cfg!(target_os = "rumprun") {
                    "rumprun-netbsd"
                } else {
                    "unknown-netbsd"
                }
            }
            "ios" => "apple-ios",
            "freebsd" => "unknown-freebsd",
            "illumos" => "unknown-illumos",

            _ => "unknown",
        };

        let env = match os.as_str() {
            "windows" => "msvc",
            "linux" => match arch.as_str() {
                "x86_64" => "gnu",
                "x86" => "gnu",
                "aarch64" => "gnu",
                "arm" => "gnueabi",
                "armv7" => "gnueabihf",
                "armv7s" => "gnueabihf",
                "mips" => "gnu",
                "mipsel" => "gnu",
                "mips64" => "gnuabi64",
                "mips64el" => "gnuabi64",
                "powerpc" => "gnu",
                "powerpc64" => "gnu",
                "powerpc64le" => "gnu",
                "riscv64gc" => "gnu",
                "s390x" => "gnu",
                "loongarch64" => "gnu",
                _ => "gnu",
            },
            "solaris" => "gnu",
            "macos" => "gnu",
            "netbsd" => "gnu",
            "ios" => "gnu",
            "freebsd" => "gnu",
            "illumos" => "gnu",
            _ => "failed",
        };

        TargetTriple::new(
            Some(arch),
            Some(os_matching.to_string()),
            Some(env.to_string()),
        )
    }
}

async fn install_rust(triple: TargetTriple, version: String) {
    let target = triple.str();
    println!("Installing Rust for target: {}", target);

    let download_url = format!(
        "https://static.rust-lang.org/dist/rust-{}-{}.tar.gz",
        version, target
    );

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template("{spinner:.green} {msg}").unwrap(),
    );

    pb.set_message("Downloading...");

    pb.enable_steady_tick(Duration::from_millis(100));

    // Download the file
    let response = reqwest::get(&download_url).await;

    if response.is_err() {
        pb.set_message("Failed to download");
        pb.finish();
        return;
    }

    pb.set_message("Unwrapping...");

    // save the file
    let file = response.unwrap().bytes().await.unwrap();

    let tar = flate2::read::GzDecoder::new(&file[..]);

    pb.set_message("Extracting...");


    let mut archive = tar::Archive::new(tar);
    // save files

    pb.set_message("Unpacking...");

    archive
        .unpack(format!("rust-{}-{}", version, triple.str()))
        .unwrap();


    pb.set_message("Running install.sh...");

    // run install.sh

    let command_res =
        tokio::process::Command::new(format!("rust-{}-{}/rust-{0}-{1}/install.sh", version, triple.str()))
            .spawn();

    if command_res.is_err() {
        pb.set_message("Failed to run install.sh");
        pb.finish();
        return;
    } else {
        pb.set_message("Done");
        pb.finish();
    
    }
}

#[tokio::main]
async fn main() {
    let target = TargetTriple::get_with_no_rust_installed();
    println!("Target triple: {}", target.str());

    let version = "1.76.0".to_string();

    install_rust(target, version).await;
}
