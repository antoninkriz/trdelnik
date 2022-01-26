use fehler::throws;
use thiserror::Error;
use tokio::{process::{Command, Child}, io::AsyncWriteExt, fs};
use std::{borrow::Cow, io, string::FromUtf8Error, process::Stdio, path::Path};
use solana_sdk::signer::keypair::Keypair;
use cargo_metadata::MetadataCommand;
use futures::future::try_join_all;
use crate::{idl::{self, Idl}, Client, program_client_generator};

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0:?}")]
    IoError(#[from] io::Error),
    #[error("{0:?}")]
    Utf8Error(#[from] FromUtf8Error),
    #[error("localnet is not running")]
    LocalnetIsNotRunning,
    #[error("localnet is still running")]
    LocalnetIsStillRunning,
    #[error("build programs failed")]
    BuildProgramsFailed,
    #[error("read program code failed: '{0}'")]
    ReadProgramCodeFailed(String),
    #[error("{0:?}")]
    IdlError(#[from] idl::Error),
}

pub struct LocalnetHandle {
    solana_test_validator_process: Child,
}

impl LocalnetHandle {
    #[throws]
    /// _Note_: Manual kill: `kill -9 $(lsof -t -i:8899)`
    pub async fn stop(mut self) {
        // @TODO Why does `await` wait indefinitely when `stop` is called by Jupyter Kernel?
        // self.solana_test_validator_process.kill().await?;
        self.solana_test_validator_process.start_kill()?;

        if Client::new(Keypair::new()).is_localnet_running(false).await {
            Err(Error::LocalnetIsStillRunning)?
        }
        fs::remove_dir_all("test-ledger").await?;
        println!("localnet stopped and its ledger deleted");
    }
}

pub struct Commander {
    root: Cow<'static, str>
}

impl Commander {
    pub fn new() -> Self {
        Self {
            root: "../../".into()
        }
    }

    pub fn with_root(root: impl Into<Cow<'static, str>>) -> Self {
        Self {
            root: root.into()
        }
    }

    #[throws]
    pub async fn build_programs(&self) {
        let success = Command::new("cargo")
            .arg("build-bpf")
            .arg("--")
            // prevent prevent dependency loop:
            // program tests -> program_client -> program
            .args(["-Z", "avoid-dev-deps"])
            .spawn()?
            .wait()
            .await?
            .success();
        if !success {
            Err(Error::BuildProgramsFailed)?;
        }
    }

    #[throws]
    pub async fn generate_program_client_lib_rs(&self) {
        let cargo_toml_data = MetadataCommand::new()
            .no_deps()
            .exec()
            .unwrap();

        let program_names = cargo_toml_data
            .packages
            .into_iter()
            .filter_map(|package| {
                match package.manifest_path.iter().nth_back(2) {
                    Some("programs") => Some(package.name),
                    _ => None,
                }
            });

        let idl_programs = program_names.map(|name| async move {
            let output = Command::new("cargo")
                .arg("+nightly")
                .arg("rustc")
                .args(["--package", &name])
                .arg("--profile=check")
                .arg("--")
                .arg("-Zunpretty=expanded")
                .output()
                .await?;
            if output.status.success() {
                let code = String::from_utf8(output.stdout)?;
                Ok(idl::parse_to_idl_program(name, &code).await?)
            } else {
                let error_text = String::from_utf8(output.stderr)?;
                Err(Error::ReadProgramCodeFailed(error_text))
            }
        });
        let idl = Idl {
            programs: try_join_all(idl_programs).await?
        };
        let program_client = program_client_generator::generate_source_code(idl);

        let mut rustfmt = Command::new("rustfmt")
            .args(["--edition", "2018"])
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        if let Some(stdio) = &mut rustfmt.stdin {
            stdio.write_all(program_client.as_bytes()).await?;
        }
        let output = rustfmt.wait_with_output().await?;
        let program_client = String::from_utf8(output.stdout)?;

        let rust_file_path = Path::new(self.root.as_ref()).join("program_client/src/lib.rs"); 
        fs::write(rust_file_path, &program_client).await?;
        println!("program_client's lib.rs regenerated")
    }

    #[throws]
    pub async fn start_localnet(&self) -> LocalnetHandle {
        let process = Command::new("solana-test-validator")
            .arg("-C")
            .arg([&self.root, "config.yml"].concat())
            .arg("-r")
            .arg("-q")
            .spawn()?;
        if !Client::new(Keypair::new()).is_localnet_running(true).await {
            Err(Error::LocalnetIsNotRunning)?
        }
        println!("localnet started");
        LocalnetHandle {
            solana_test_validator_process: process,
        }
    }
}
