# URL Ferry

A two-part solution for forwarding HTTP/HTTPS links from Discord (when running as a different Windows user) to your main user, so links open in your default browser on the main user instead.

## Architecture

**Two binaries:**

1. **url-ferry-listener** (runs as main user)
   - Listens on a named pipe (`\\.\pipe\url_ferry`) for URLs
   - Launches URLs using the system's default handler (respects existing browser windows)
   - Runs silently in the background (GUI subsystem, no console window)
   - Installed to autorun for convenience

2. **url-ferry-sender** (runs as DiscordUser)
   - Registered as the handler for `http://` and `https://` protocol links
   - Sends clicked URLs to the listener via the named pipe
   - Shows error notifications if the listener isn't running
   - Also runs as GUI (no console)

## Installation & Setup

### Installation

1. Put both binaries in a folder accessible to both users (e.g., `C:\Program Files\URL Ferry`).

### Main User Side

1. Run the listener with autorun installation:

   ```
   url-ferry-listener.exe --install
   ```

   This registers the listener to start automatically on login. Optionally, add logging:

   ```
   url-ferry-listener.exe --install --log C:\path\to\log.txt
   ```

2. Restart your computer, or manually start the listener:

   ```
   url-ferry-listener.exe
   ```

   The listener will run silently (no console window) and wait for URLs.

### DiscordUser Side

1. Register the sender as the default handler for http/https. Open a terminal (Command Prompt or PowerShell) as your main user and run:

   ```
   runas /user:DiscordUser "url-ferry-sender.exe --register"
   ```
   You will be prompted for the DiscordUser's password.

2. Switch to DiscordUser in order to change the default protocol handler for `http` and `https`. Do this in Settings > Apps > Default Apps > Choose default apps by protocol, and set the `http` and `https` protocols to use `url-ferry-sender`. This is necessary because the protocol registration requires user consent.

3. Test it: Click any HTTP/HTTPS link in Discord. It should open in your main user's default browser.

## Uninstallation

### Main User
```
url-ferry-listener.exe --uninstall
```

### DiscordUser
```
runas /user:DiscordUser "url-ferry-sender.exe --unregister"
```

## Protocol Details

- **IPC Mechanism:** Named pipes (`\\.\pipe\url_ferry`)
- **Message Format:** NUL-terminated UTF-8 strings
- **URL Validation:** Only accepts `http://` and `https://` URLs
- **Buffer Size:** 4096 bytes (supports very long URLs)

## Error Handling

- If the listener isn't running or the pipe is unavailable, the sender shows an error MessageBox.
- The listener logs errors to the specified log file (if `--log` is provided).
- Both binaries fail gracefully without crashing the system.

## Security Model

- The named pipe is accessible to the Users group (read/write permissions).
- No URL filtering beyond protocol validation.
- URLs are sent in plaintext over the named pipe (secure only because DiscordUser and main user are on the same machine).

## Building from Source

Requirements: Rust 1.70+, Windows 10/11

```bash
cargo build --release
```

Binaries are in `target/release/`:
- `url-ferry-listener.exe`
- `url-ferry-sender.exe`

## Configuration

### Logging

Enable debug logging on the listener to troubleshoot:

```
url-ferry-listener.exe --log C:\Temp\url-ferry.log
```

Log format: `[YYYY-MM-DD HH:MM:SS.mmm] <message>`

Each log entry includes timestamps, the URL attempted, and success/failure status.

## Troubleshooting

**Links don't open in Discord:**
- Verify the listener is running: Check Task Manager or open the listener manually.
- Check protocol handler registration: Confirm `url-ferry-sender.exe --register` ran successfully.
- Look at the log file if enabled: `url-ferry-listener.exe --log C:\Temp\url-ferry.log`

**Manual Testing Without Discord**

Test the URL forwarding directly (useful for debugging):

```cmd
# Start listener in one window
url-ferry-listener.exe --log test.log

# In another window, send a test URL
url-ferry-sender.exe --send "https://example.com"
```

Expected output: `✓ URL sent to listener: https://example.com`

This will open the URL in your default browser (on the listener's user account) without needing Discord or protocol registration.

**Listener closes immediately:**
- This shouldn't happen; if it does, run it manually to see error output and/or enable logging to capture issues.

**Error notifications appear when clicking links:**
- The listener probably isn't running. Start it manually or reboot (it should be in autorun).

## Performance

- Binary sizes: ~1MB each
- Startup time: <100ms
- URL forwarding: <50ms typical latency

## License

MIT
