# 📺 Screen Streaming — Simple Guide

This explains how to turn your screen sharing **ON** and **OFF**, and how to
watch it. No tech knowledge needed. Just follow the steps.

> **What this does:** when it's ON, you can open a web page (on this PC or your
> phone) and see this computer's screen, live.

---

## ⭐ THE EASIEST WAY — use the "Services" app

This is the simplest way to turn it on and off. You just click buttons.

### Open the Services app (do this first)
1. Press the **Windows key** + **R** together. A small box pops up.
2. Type:  **`services.msc`**
3. Press **Enter**.
4. A window opens with a big list of names. Scroll down to find
   **"Screen Streaming Service"**. (The list is alphabetical — look under **S**.)

Keep this window open. From here you can start and stop it.

---

## ▶️ TO TURN IT ON (start sharing your screen)

1. In the Services app, **click once** on **"Screen Streaming Service"** to select it.
2. On the left side, click the blue **"Start"** link.
   *(Or right-click the name → click **Start**.)*
3. Wait 2–3 seconds. The **Status** column should now say **"Running"**.

✅ That's it — your screen is now being shared.

**Now watch it:**
- **On this PC:** open your web browser, type **`localhost:8080`** in the address
  bar, press Enter.
- **On your phone:** see the "Watch on your phone" section below.

---

## ⏹️ TO TURN IT OFF (stop sharing)

1. In the Services app, **click once** on **"Screen Streaming Service"**.
2. On the left side, click the blue **"Stop"** link.
   *(Or right-click the name → click **Stop**.)*
3. The **Status** column goes blank (no longer "Running").

✅ Screen sharing is now off. The web page will stop showing your screen.

> ⚠️ **IMPORTANT:** This **"Stop"** button in the Services app is the **ONLY**
> correct way to turn it off. If you try to close it any other way (like "End
> task" in Task Manager), it will **turn itself back on automatically**. That's
> on purpose — so it never stops by accident. Use **Services → Stop**.

---

## ❓ HOW TO CHECK IF IT'S ON OR OFF

Open the Services app (Windows key + R → `services.msc` → Enter), find
**"Screen Streaming Service"**, and look at the **Status** column:
- Says **"Running"** = it's **ON**.
- **Blank** = it's **OFF**.

---

## 📱 WATCH ON YOUR PHONE

Your phone must be on the **same Wi-Fi** as this computer.

**One-time setup (only the very first time):** the firewall needs to be opened.
Do this once:
1. Click the **Start menu**, type **`powershell`**.
2. **Right-click** "Windows PowerShell" → **"Run as administrator"** → click **Yes**.
3. Copy-paste this line and press Enter:
   ```
   New-NetFirewallRule -DisplayName "Screen Stream" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
   ```
4. Close that window. You never have to do this again.

**Every time you want to watch on your phone:**
1. Make sure the service is **ON** (see "Turn it on" above).
2. On your phone's web browser, type:  **`192.168.0.109:8080`**
3. Your screen appears.

> If `192.168.0.109` doesn't work, this PC's address may have changed. To find
> the new one: Start menu → type `cmd` → Enter → type `ipconfig` → Enter → look
> for **"IPv4 Address"** under your Wi-Fi (it looks like `192.168.x.x`). Use that
> number with `:8080` on the phone.

---

## 🔴 SHOULD IT TURN ON BY ITSELF WHEN I START THE PC?

Right now, **no** — you turn it on yourself each time (that's safer).

If you'd rather it **start automatically every time you turn on the computer**,
tell me and I'll change one setting for you. (Or, in the Services app: double-click
"Screen Streaming Service" → change **"Startup type"** to **Automatic** → OK.)

---

## 🆘 IF SOMETHING GOES WRONG

**The web page says "can't connect" / "refused":**
The service is OFF. Turn it on (Services app → Start).

**The page is black / frozen:**
Wait a few seconds and refresh the page. If still black, turn it OFF then ON
again (Services → Stop, wait 3 seconds, Start).

**Phone won't connect:**
- Is the phone on the same Wi-Fi as the PC?
- Is the service ON?
- Did you do the one-time firewall step above?

**Still stuck?** There are log files that explain what happened, here:
`C:\ProgramData\ss-service\ss-service.log` and `ss-agent.log`. Send them to me.

---

## 🧹 IF YOU EVER WANT TO REMOVE IT COMPLETELY

Right-click **`uninstall.bat`** in the project folder → **"Run as administrator"**. Done.

---

## 🔧 ONLY IF I TELL YOU TO REINSTALL (after a code change)

Right-click **`install.bat`** in the project folder → **"Run as administrator"**, answer
**`y`** when it asks. Then turn it on the normal way (Services → Start).
