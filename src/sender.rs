#![windows_subsystem = "windows"]

mod common;

use clap::Parser;
use common::{validate_url, get_exe_path, Result, PIPE_NAME, NUL_TERMINATOR, UrlFerryError};
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use winreg::RegKey;

// File open mode constant
const FILE_OPEN_EXISTING: u32 = 3;

#[derive(Parser, Debug)]
#[command(name = "url-ferry-sender")]
#[command(about = "Protocol handler for http/https that forwards URLs via named pipe", long_about = None)]
struct Args {
    /// URL to forward (typically passed by Windows as %1)
    url: Option<String>,

    /// Send a URL directly to the listener (for testing)
    #[arg(long)]
    send: Option<String>,

    /// Register this binary as the default http/https protocol handler
    #[arg(long)]
    register: bool,

    /// Unregister this binary as the protocol handler
    #[arg(long)]
    unregister: bool,
}

fn register_protocol_handler() -> Result<()> {
    let exe_path = get_exe_path()?;
    let exe_path_str = exe_path.to_string_lossy().to_string();
    let handler_path = format!("\"{}\" \"%1\"", exe_path_str);
    let icon_path = format!("\"{}\",0", exe_path_str);

    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);

    // Remove any legacy direct protocol overrides from earlier versions.
    let _ = hkcu.delete_subkey_all("Software\\Classes\\http\\shell\\open\\command");
    let _ = hkcu.delete_subkey_all("Software\\Classes\\https\\shell\\open\\command");

    // 1. Register in App Paths
    let (app_paths_key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\url-ferry-sender.exe")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create App Paths key: {:?}", e)))?;
    app_paths_key.set_value("", &exe_path_str)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set default value in App Paths: {:?}", e)))?;
    app_paths_key.set_value("UseUrl", &1u32)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set UseUrl: {:?}", e)))?;
    app_paths_key.set_value("SupportedProtocols", &"http:https".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set SupportedProtocols: {:?}", e)))?;
    app_paths_key.set_value("DefaultIcon", &icon_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set DefaultIcon in App Paths: {:?}", e)))?;

    // 2. Create ProgIDs for http and https
    // HTTP ProgID
    let (http_progid_key, _) = hkcu.create_subkey("Software\\Classes\\url-ferry.http")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create http ProgID: {:?}", e)))?;
    http_progid_key.set_value("URL Protocol", &"".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set URL Protocol: {:?}", e)))?;
    http_progid_key.set_value("DefaultIcon", &icon_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set DefaultIcon for http: {:?}", e)))?;

    let (http_shell_open, _) = hkcu.create_subkey("Software\\Classes\\url-ferry.http\\shell\\open\\command")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create http shell command: {:?}", e)))?;
    http_shell_open.set_value("", &handler_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set http command handler: {:?}", e)))?;

    // HTTPS ProgID
    let (https_progid_key, _) = hkcu.create_subkey("Software\\Classes\\url-ferry.https")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create https ProgID: {:?}", e)))?;
    https_progid_key.set_value("URL Protocol", &"".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set URL Protocol: {:?}", e)))?;
    https_progid_key.set_value("DefaultIcon", &icon_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set DefaultIcon for https: {:?}", e)))?;

    let (https_shell_open, _) = hkcu.create_subkey("Software\\Classes\\url-ferry.https\\shell\\open\\command")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create https shell command: {:?}", e)))?;
    https_shell_open.set_value("", &handler_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set https command handler: {:?}", e)))?;

    // 3. Create Capabilities tree
    let (capabilities_key, _) = hkcu.create_subkey("Software\\UrlFerry\\Capabilities")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create Capabilities key: {:?}", e)))?;
    capabilities_key.set_value("ApplicationName", &"URL Ferry".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set ApplicationName: {:?}", e)))?;
    capabilities_key.set_value("ApplicationDescription", &"Forward http/https from DiscordUser to the main user's browser.".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set ApplicationDescription: {:?}", e)))?;
    capabilities_key.set_value("DefaultIcon", &icon_path)
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set DefaultIcon in Capabilities: {:?}", e)))?;

    // Create UrlAssociations subkey
    let (url_assoc_key, _) = hkcu.create_subkey("Software\\UrlFerry\\Capabilities\\UrlAssociations")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create UrlAssociations key: {:?}", e)))?;
    url_assoc_key.set_value("http", &"url-ferry.http".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set http association: {:?}", e)))?;
    url_assoc_key.set_value("https", &"url-ferry.https".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to set https association: {:?}", e)))?;

    // 4. Register in RegisteredApplications
    let (reg_apps_key, _) = hkcu.create_subkey("Software\\RegisteredApplications")
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to create RegisteredApplications key: {:?}", e)))?;
    reg_apps_key.set_value("URL Ferry", &"Software\\UrlFerry\\Capabilities".to_string())
        .map_err(|e| UrlFerryError::RegistryError(format!("Failed to register in RegisteredApplications: {:?}", e)))?;

    Ok(())
}

fn unregister_protocol_handler() -> Result<()> {
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);

    // Delete all registry entries created during registration
    let entries_to_delete = vec![
        "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\url-ferry-sender.exe",
        "Software\\Classes\\url-ferry.http",
        "Software\\Classes\\url-ferry.https",
        "Software\\UrlFerry",
        "Software\\Classes\\http\\shell\\open\\command",
        "Software\\Classes\\https\\shell\\open\\command",
    ];

    for entry in entries_to_delete {
        if hkcu.open_subkey(entry).is_ok() {
            let _ = hkcu.delete_subkey_all(entry);
        }
    }

    // Remove from RegisteredApplications
    if let Ok(reg_apps_key) = hkcu.open_subkey("Software\\RegisteredApplications") {
        let _ = reg_apps_key.delete_value("URL Ferry");
    }

    Ok(())
}

fn send_url_to_listener(url: &str) -> Result<()> {
    validate_url(url)?;

    unsafe {
        let pipe_name = std::ffi::OsStr::new(PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();

        // Try to open the pipe
        let handle = match CreateFileW(
            windows::core::PCWSTR(pipe_name.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            FILE_CREATION_DISPOSITION(FILE_OPEN_EXISTING),
            FILE_ATTRIBUTE_NORMAL,
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                return Err(UrlFerryError::PipeError(
                    "Listener not running or pipe unavailable".to_string(),
                ));
            }
        };

        // Write the URL with NUL terminator
        let url_bytes = url.as_bytes();
        let mut to_write = url_bytes.to_vec();
        to_write.push(NUL_TERMINATOR);

        let result = match WriteFile(
            handle,
            Some(&to_write[..]),
            None,
            None,
        ) {
            Ok(_) => Ok(()),
            Err(_) => {
                Err(UrlFerryError::PipeError(
                    "Failed to write to named pipe".to_string(),
                ))
            }
        };

        let _ = CloseHandle(handle);
        result
    }
}

fn show_error_notification(message: &str) {
    unsafe {
        let title = std::ffi::OsStr::new("URL Forwarder Error")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();

        let msg = std::ffi::OsStr::new(message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();

        // Show MessageBox
        let _ = MessageBoxW(
            None,
            windows::core::PCWSTR(msg.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_ICONERROR | MB_OK | MB_SETFOREGROUND,
        );
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.register {
        register_protocol_handler()?;
        println!("✓ Registered URL Ferry. Please go to Settings > Apps > Default apps and search for 'URL Ferry' to set it as your default handler for http/https.");
        return Ok(());
    }

    if args.unregister {
        unregister_protocol_handler()?;
        println!("✓ Unregistered URL Ferry.");
        return Ok(());
    }

    // Handle explicit --send for testing
    if let Some(url) = args.send {
        match send_url_to_listener(&url) {
            Ok(_) => {
                println!("✓ URL sent to listener: {}", url);
            }
            Err(e) => {
                let error_msg = match e {
                    UrlFerryError::PipeError(msg) => {
                        format!("Could not connect to listener: {}", msg)
                    }
                    UrlFerryError::InvalidUrl(msg) => {
                        format!("Invalid URL: {}", msg)
                    }
                    _ => {
                        format!("Error: {:?}", e)
                    }
                };
                eprintln!("✗ {}", error_msg);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if let Some(url) = args.url {
        match send_url_to_listener(&url) {
            Ok(_) => {
                // Silent success for protocol handler
            }
            Err(e) => {
                let error_msg = match e {
                    UrlFerryError::PipeError(msg) => {
                        format!("Could not connect to URL Forwarder listener.\n\nMake sure the listener is running.\n\nError: {}", msg)
                    }
                    UrlFerryError::InvalidUrl(msg) => {
                        format!("Invalid URL: {}", msg)
                    }
                    _ => {
                        format!("Error forwarding URL: {:?}", e)
                    }
                };
                show_error_notification(&error_msg);
            }
        }
    } else {
        eprintln!("No URL provided. Use one of:");
        eprintln!("  url-ferry-sender --send <url>       (test mode)");
        eprintln!("  url-ferry-sender --register         (setup protocol handler)");
        eprintln!("  url-ferry-sender --unregister       (remove protocol handler)");
        eprintln!("  url-ferry-sender <url>              (called by Windows protocol handler)");
        std::process::exit(1);
    }

    Ok(())
}
