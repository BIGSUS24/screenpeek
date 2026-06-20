# How to Run the Screen Streaming Service

This runs as a **real Windows service** — invisible, in the background, listed in
`services.msc` / Task Manager's *Services* tab. It captures **everything**
(your desktop, and — because it runs as SYSTEM — the **login screen, lock screen,
and UAC prompts**) and serves it on `http://localhost:8080`.

> ⚠️ **No password.** Anyone on your Wi-Fi who opens the link sees your screen.
> It's built for personal use on a trusted network. Don't run it on public Wi-Fi.

---

## How it works (why it's a service + an agent)

A Windows service runs in "Session 0", which is walled off from your desktop and
**cannot capture the screen** by itself. So:

```
[SSService - LocalSystem, Session 0]      invisible, auto-restarts
   ├─ web server        http://localhost:8080
   ├─ frame intake      127.0.0.1:8765
   └─ launches a capture AGENT into your active desktop session, as SYSTEM
                 │
                 ▼
[capture agent - SYSTEM, your session]    follows the input desktop
   └─ captures desktop + lock/login/secure desktop -> JPEG -> back to service
```

The service keeps the agent alive and re-launches it when you log on/off, lock,
or switch users. The agent is tied to the service (job object), so it can never
be left running on its own.

---

## Install (one time, as administrator)

1. **Right-click `install.bat` → "Run as administrator".**
   It builds, copies to `C:\ProgramData\ss-service\`, creates config, grants
   consent (answer `y`), and registers the service as **SYSTEM** with auto-restart.
2. When it says *Installation Complete*, **start it**:
   ```
   sc start SSService
   ```
   (or Task Manager → *Services* tab → right-click `SSService` → **Start**)
3. Open <http://localhost:8080>.

---

## Turn it ON / OFF

| Action | How |
|--------|-----|
| **Start** (you trigger it) | `sc start SSService`  — or Services tab → Start |
| **Stop** (only you can) | Task Manager → **Services** tab → right-click `SSService` → **Stop**  (or `sc stop SSService` as admin) |
| **Check** | `sc query SSService` |

> 🔒 **Only you can stop it.** It runs as SYSTEM, so no normal app or non-admin
> user can stop it, and if anything *kills* the process it **auto-restarts**.
> **"End task" in the Processes tab will NOT stop it** — it just respawns. To
> actually stop it, use the **Services tab → Stop** (a deliberate stop does not
> trigger the restart).

**Start automatically at every boot** (instead of starting it yourself):
```
sc config SSService start= auto
```
Back to manual: `sc config SSService start= demand`

---

## Watch from your phone (same Wi-Fi)

1. **Open the firewall once** (Administrator PowerShell):
   ```powershell
   New-NetFirewallRule -DisplayName "SS Screen Stream 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
   ```
2. Find this PC's IP: run `ipconfig`, look at the Wi-Fi adapter's *IPv4 Address*
   (it was `192.168.0.109` last time).
3. On the phone's browser: `http://192.168.0.109:8080`

Remove the firewall rule later:
`Remove-NetFirewallRule -DisplayName "SS Screen Stream 8080"`

---

## File locations

| What | Path |
|------|------|
| Service + agent (same exe) | `C:\ProgramData\ss-service\ss-service.exe` |
| Admin CLI | `C:\ProgramData\ss-service\ss-cli.exe` |
| Config (ports, fps, monitor) | `C:\ProgramData\ss-service\config.toml` |
| Service log | `C:\ProgramData\ss-service\ss-service.log` |
| Agent log (capture) | `C:\ProgramData\ss-service\ss-agent.log` |
| Consent record | `C:\ProgramData\ss-service\consent.bin` |
| Viewer web page | `C:\ProgramData\ss-service\index.html` |
| Source code | `crates/` (in project root) |

---

## Change settings

Edit `C:\ProgramData\ss-service\config.toml`, then restart the service
(`sc stop SSService` → `sc start SSService`).

| Setting | Key | Default |
|---------|-----|---------|
| Web port | `[server] http_port` | `8080` |
| Frame rate | `[capture] fps` | `30` |
| Which monitor | `[capture] monitor_index` | `0` |

---

## Uninstall

**Right-click `uninstall.bat` → "Run as administrator".** It clears the
auto-restart policy, stops and kills the service + agent, deletes the service,
and removes `C:\ProgramData\ss-service\`.

---

## Troubleshooting

Check the logs first: `C:\ProgramData\ss-service\ss-service.log` and `ss-agent.log`.

**Page won't load (`ERR_CONNECTION_REFUSED`)**
Service isn't started. `sc start SSService`, then check `sc query SSService` shows RUNNING.

**Page loads but the screen is blank / frozen**
The capture agent may not have launched into your session yet. Check `ss-agent.log`.
It can take a second after logon; the viewer auto-reconnects.

**Lock screen shows but doesn't update**
Capturing the secure desktop requires the agent to be running as SYSTEM (it is,
when launched by the service). If it doesn't work, check `ss-agent.log` for
`OpenInputDesktop failed` / `SetThreadDesktop failed`.

**Phone can't connect**
Firewall rule missing, or the phone isn't on the same Wi-Fi, or your network is
set to "Public" (the rule above is for "Private" — change `-Profile Private` to
`-Profile Any` if needed).

---

## Rebuild after code changes

```
cargo build --release --bin ss-service --bin ss-cli
```
Then re-run `install.bat` as admin to redeploy.
