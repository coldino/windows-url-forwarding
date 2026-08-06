#![windows_subsystem = "windows"]

mod common;

use clap::Parser;
use common::{validate_url, UrlFerryError, Result, PIPE_NAME, NUL_TERMINATOR};
use std::fs::File;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Pipes::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
use windows::Win32::Security::*;
use windows::core::PCWSTR;
use winreg::RegKey;

#[derive(Parser, Debug)]
#[command(name = "url-ferry-listener")]
#[command(about = "Listen for URLs from url-ferry-sender and launch them", long_about = None)]
struct Args {
    #[arg(long)]
    install: bool,

    #[arg(long)]
    uninstall: bool,

    #[arg(long)]
    log: Option<PathBuf>,
}

struct Logger {
    file: Option<File>,
}

impl Logger {
    fn new(log_path: Option<PathBuf>) -> Result<Self> {
        let file = if let Some(path) = log_path {
            Some(File::create(&path)?)
        } else {
            None
        };
        Ok(Logger { file })
    }

    fn log(&mut self, message: &str) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_line = format!("[{}] {}\n", timestamp, message);

        if let Some(ref mut f) = self.file {
            let _ = f.write_all(log_line.as_bytes());
            let _ = f.flush();
        }
    }
}

fn install_autorun() -> Result<()> {
    let exe_path = std::env::current_exe()?;
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (run_key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| UrlFerryError::RegistryError(format!("{:?}", e)))?;

    run_key.set_value("URLForwarder", &exe_path.to_string_lossy().to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("{:?}", e)))?;

    Ok(())
}

fn uninstall_autorun() -> Result<()> {
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| UrlFerryError::RegistryError(format!("{:?}", e)))?;
    run_key.delete_value("URLForwarder")
        .map_err(|e| UrlFerryError::RegistryError(format!("{:?}", e)))?;
    Ok(())
}

unsafe fn create_named_pipe() -> Result<HANDLE> {
    let pipe_name = std::ffi::OsStr::new(PIPE_NAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    // Create a security descriptor that allows all users to access the pipe
    let mut sd: [u8; std::mem::size_of::<SECURITY_DESCRIPTOR>()] = [0; std::mem::size_of::<SECURITY_DESCRIPTOR>()];
    let sd_ptr = sd.as_mut_ptr() as *mut SECURITY_DESCRIPTOR;

    // Initialize the security descriptor (revision 1)
    InitializeSecurityDescriptor(PSECURITY_DESCRIPTOR(sd_ptr as *mut _), 1)
        .map_err(|_| UrlFerryError::PipeError(
            "Failed to initialize security descriptor".to_string(),
        ))?;

    // Set NULL DACL (allows everyone full access)
    // This is safe because both users (main + DiscordUser) are on the same machine
    SetSecurityDescriptorDacl(
        PSECURITY_DESCRIPTOR(sd_ptr as *mut _),
        true,
        None,
        false,
    )
    .map_err(|_| UrlFerryError::PipeError(
        "Failed to set DACL".to_string(),
    ))?;

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_ptr as *mut _,
        bInheritHandle: false.into(),
    };

    let handle = CreateNamedPipeW(
        PCWSTR(pipe_name.as_ptr()),
        FILE_FLAG_FIRST_PIPE_INSTANCE | PIPE_ACCESS_INBOUND,
        NAMED_PIPE_MODE(0),
        1,
        0,
        0,
        0,
        Some(&sa),
    );

    Ok(handle)
}

unsafe fn connect_pipe(handle: HANDLE) -> Result<()> {
    match ConnectNamedPipe(handle, None) {
        Ok(_) => Ok(()),
        Err(_e) => {
            // Pipe already connected is OK
            Ok(())
        }
    }
}

unsafe fn read_url_from_pipe(handle: HANDLE) -> Result<Option<String>> {
    let mut buffer = [0u8; 4096];
    let mut bytes_read = 0u32;

    match ReadFile(
        handle,
        Some(&mut buffer[..]),
        Some(&mut bytes_read),
        None,
    ) {
        Ok(_) => {
            if bytes_read == 0 {
                return Ok(None);
            }

            let data = &buffer[..bytes_read as usize];

            // Find the NUL terminator
            if let Some(nul_pos) = data.iter().position(|&b| b == NUL_TERMINATOR) {
                let url_bytes = &data[..nul_pos];
                let url = String::from_utf8(url_bytes.to_vec())
                    .map_err(|e| UrlFerryError::InvalidUrl(format!("Invalid UTF-8: {}", e)))?;
                Ok(Some(url))
            } else {
                Err(UrlFerryError::InvalidUrl(
                    "URL not NUL-terminated".to_string(),
                ))
            }
        }
        Err(e) => Err(UrlFerryError::PipeError(format!("ReadFile failed: {:?}", e))),
    }
}

fn launch_url(url: &str) -> Result<()> {
    validate_url(url)?;

    use std::ffi::OsStr;

    let url_wide: Vec<u16> = OsStr::new(url)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let operation = OsStr::new("open")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(url_wide.as_ptr()),
            PCWSTR(std::ptr::null()),
            PCWSTR(std::ptr::null()),
            SW_SHOW,
        );

        // ShellExecuteW returns an HINSTANCE; anything > 32 is success
        if result.0 as usize <= 32 {
            Err(UrlFerryError::LaunchError(format!(
                "ShellExecute failed with code: {}",
                result.0 as usize
            )))
        } else {
            Ok(())
        }
    }
}

fn listen_loop(mut logger: Logger) -> Result<()> {
    loop {
        unsafe {
            match create_named_pipe() {
                Ok(pipe) => {
                    let _ = connect_pipe(pipe);

                    match read_url_from_pipe(pipe) {
                        Ok(Some(url)) => {
                            match launch_url(&url) {
                                Ok(_) => {
                                    logger.log(&format!("Launched URL: {}", url));
                                }
                                Err(e) => {
                                    let err_msg = format!("Failed to launch URL '{}': {:?}", url, e);
                                    logger.log(&err_msg);
                                }
                            }
                        }
                        Ok(None) => {
                            logger.log("Received empty message");
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to read URL: {:?}", e);
                            logger.log(&err_msg);
                        }
                    }

                    let _ = DisconnectNamedPipe(pipe);
                    let _ = windows::Win32::Foundation::CloseHandle(pipe);
                }
                Err(e) => {
                    logger.log(&format!("Failed to create pipe: {:?}", e));
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.install {
        install_autorun()?;
        println!("URL Forwarder listener installed to autorun");
        return Ok(());
    }

    if args.uninstall {
        uninstall_autorun()?;
        println!("URL Forwarder listener removed from autorun");
        return Ok(());
    }

    let logger = Logger::new(args.log)?;
    listen_loop(logger)?;

    Ok(())
}
