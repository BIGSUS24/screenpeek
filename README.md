<div align="center">

# Screenpeek

### See your screen from anywhere on your network

A lightweight, privacy-first screen sharing service for Windows.
Share your desktop with any device on your LAN — phone, tablet, another PC —
just open a browser. No installs. No accounts. No cloud.

[![Rust](https://img.shields.io/badge/Built_with-Rust-orange?logo=rust)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Windows](https://img.shields.io/badge/Platform-Windows_10%2F11-blue?logo=windows)](https://microsoft.com/windows)

</div>

---

## What is Screenpeek?

Screenpeek captures your Windows desktop and streams it live to any web browser on your
local network. It runs as a Windows service (or console app), uses DXGI for hardware-accelerated
capture, JPEG encoding with zero native dependencies, and serves the stream over plain HTTP.

**Open your phone's browser. Type `your-pc-ip:8080`. See your screen.** That's it.

---

## Features

- **Zero config** — one-click install, works out of the box
- **Privacy-first** — consent-based; screen capture is off until you explicitly grant permission
- **No cloud, no accounts** — everything stays on your local network
- **Lightweight** — pure Rust, tiny binary, minimal resource usage
- **Works on any device** — phone, tablet, laptop, Smart TV — any browser with Wi-Fi
- **Token-based auth** — HMAC-signed, time-limited access tokens
- **HTTPS support** — built-in TLS certificate generation
- **Configurable** — change ports, FPS, max viewers, monitor index
- **One-click uninstall** — clean removal, no leftovers

---

## Quick Start

### Prerequisites

- Windows 10 or 11
- [Rust](https://rustup.rs) installed (`cargo --version` should work)
- Administrator access (for service install)

### Install

```bash
# Clone the repo
git clone https://github.com/bigsus24/screenpeek.git
cd screenpeek

# Right-click install.bat → Run as administrator
```

The installer will:
1. Build the release binaries (first build takes a few minutes)
2. Copy them to `C:\ProgramData\ss-service\`
3. Run setup and create config
4. Register the Windows service

### First Run (4 quick steps)

Open an **admin** Command Prompt:

```bat
# 1. Grant screen capture permission
"C:\ProgramData\ss-service\ss-cli.exe" consent grant

# 2. Generate TLS certificate
"C:\ProgramData\ss-service\ss-cli.exe" generate-tls

# 3. Start the service
sc start SSService

# 4. Open in browser
start http://localhost:8080
```

> **Tip:** Use console mode for screen capture to work:
> ```bat
> sc stop SSService
> "C:\ProgramData\ss-service\ss-service.exe" --console
> ```

---

## Watch on Your Phone

1. Make sure your phone is on the **same Wi-Fi** as your PC
2. Start the service (see above)
3. Open your phone's browser
4. Type: `192.168.x.x:8080` (use your PC's IP — run `ipconfig` to find it)

**First time?** Open firewall port 8080 once:
```powershell
# Run in admin PowerShell
New-NetFirewallRule -DisplayName "Screen Stream" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
```

---

## Configuration

Edit `C:\ProgramData\ss-service\config.toml`:

| Setting       | Default       | Description                    |
|---------------|---------------|--------------------------------|
| HTTP port     | `8080`        | Viewer page & stream endpoint  |
| HTTPS port    | `8443`        | TLS endpoint                   |
| Max viewers   | `3`           | Concurrent browser connections |
| FPS           | `30`          | Capture frame rate             |
| Monitor       | `0`           | Which monitor to capture       |

Restart the service after changes:
```bat
sc stop SSService && sc start SSService
```

---

## Building from Source

```bash
cargo build --release --bin ss-service --bin ss-cli
```

Binaries appear in `target/release/`.

---

## Uninstall

Right-click `uninstall.bat` → Run as administrator, or:

```bat
sc stop SSService
sc delete SSService
rmdir /s /q "C:\ProgramData\ss-service"
```

---

## How It Works

```
Windows DXGI ──► Capture Frame (BGRA) ──► JPEG Encoder ──► Broadcast Channel
                                                                  │
   Browser ──GET /────────────► HTTP Server (8080) ──► Viewer Page
   Browser ──POST /api/token──► HMAC-signed token (time-limited)
   Browser ──GET /stream?token=…──► Verify ──► Subscribe to frames
   Browser ◄── multipart/x-mixed-replace JPEG stream ◄── <img> tag
```

**Architecture** — modular Rust workspace:

| Crate       | Purpose                              |
|-------------|--------------------------------------|
| `core`      | Config, crypto, consent, tokens      |
| `capture`   | DXGI desktop duplication             |
| `encoder`   | JPEG encoding (pure Rust)            |
| `stream`    | Frame pipeline & session management  |
| `server`    | HTTP server, viewer page, TLS        |
| `service`   | Windows service entry point          |
| `cli`       | Admin CLI (setup, consent, status)   |

---

## Limitations

- **Local network only** — no NAT traversal (use VPN or reverse proxy for remote access)
- **MJPEG** — bandwidth-heavy compared to H.264; fine on LAN
- **Video only** — no audio support yet
- **Windows only** — DXGI capture requires Windows

---

## Contributing

Contributions welcome! Fork the repo, create a feature branch, and open a PR.

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Built with care in Rust**

[Report a Bug](https://github.com/bigsus24/screenpeek/issues) · [Request a Feature](https://github.com/bigsus24/screenpeek/issues)

</div>
