//! Electris rust integration inside the CI
use core::cell::RefCell;

use clightning_testing::check_dir_or_make_if_missing;
use port::Port;
use port_selector as port;
use tempfile::TempDir;

pub mod macros {
    #[macro_export]
    macro_rules! electrs {
        ($dir:expr, $btcport:expr, $port:expr, $($opt_args:tt)*) => {
            async {
                use tokio::process::Command;

                let opt_args = format!($($opt_args)*);
                let args = opt_args.trim();
                let args_tok: Vec<&str> = args.split(" ").collect();

                let path = format!("{}/.electrs", $dir.path().to_str().unwrap());
                log::info!("electrs home {path}");
                check_dir_or_make_if_missing(path.clone()).await.unwrap();
                let mut command = Command::new("electrs");
                command
                    .args(&args_tok)
                    .arg(format!("--electrum-rpc-addr=127.0.0.1:{}", $port))
                    .arg(format!("--daemon-rpc-addr=127.0.0.1:{}", $btcport))
                    .arg(format!("--db-dir={path}"))
                    .stdout(std::process::Stdio::null())
                    .spawn()
            }.await
        };
        ($dir:expr, $btcport:expr, $port:expr) => {
            $crate::electrs!($dir, $btcport, $port, "")
        };
    }

    pub use electrs;
}

pub struct ElectrsNode {
    pub port: Port,
    root_path: TempDir,
    process: RefCell<Vec<tokio::process::Child>>,
}

impl Drop for ElectrsNode {
    fn drop(&mut self) {
        for process in self.process.borrow().iter() {
            let Some(child) = process.id() else {
                continue;
            };
            let Ok(mut kill) = std::process::Command::new("kill")
                .args(["-s", "SIGKILL", &child.to_string()])
                .spawn()
            else {
                continue;
            };
            let _ = kill.wait();
        }

        let result = std::fs::remove_dir_all(self.root_path.path());
        log::debug!(target: "btc", "clean up function {:?}", result);
    }
}

impl ElectrsNode {
    pub async fn tmp(network: &str, btc_port: u64) -> anyhow::Result<Self> {
        let dir = tempfile::tempdir()?;
        let port = port::random_free_port().unwrap();
        // FIXME: This required the cookie, but I am not sure
        // that it is possible
        let process = macros::electrs!(dir, btc_port, port, "--network={network}")?;

        let bg_process = vec![process];
        Ok(Self {
            root_path: dir,
            port,
            process: bg_process.into(),
        })
    }
}
