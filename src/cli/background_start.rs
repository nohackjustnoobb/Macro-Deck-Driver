use std::{
    env,
    process::{Command, Stdio},
};

pub fn background_start(
    port: Option<String>,
    config_path: Option<String>,
    tcp_port: Option<String>,
) {
    let exe_path = match env::current_exe() {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Failed to get current exe path");
            return;
        }
    };

    let mut command = Command::new(exe_path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("start")
        .arg("--foreground");

    if let Some(port) = port {
        command.arg("--port").arg(port);
    }

    if let Some(config_path) = config_path {
        command.arg("--config-path").arg(config_path);
    }

    if let Some(tcp_port) = tcp_port {
        command.arg("--tcp-port").arg(tcp_port);
    }

    // TODO not tested
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        if command
            .spawn()
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
            .spawn()
            .is_err()
        {
            eprintln!("Failed to start background process");
            return;
        }
    }

    #[cfg(unix)]
    {
        use nix::unistd::setsid;
        use std::{
            io::{Error, ErrorKind},
            os::unix::process::CommandExt,
        };

        unsafe {
            if command
                .pre_exec(|| match setsid() {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::new(ErrorKind::Other, e)),
                })
                .spawn()
                .is_err()
            {
                eprintln!("Failed to start background process");
                return;
            }
        }
    }

    #[cfg(not(any(windows, unix)))]
    {
        eprintln!("Unsupported platform");
        return;
    }

    println!("Start listening in the background");
}
