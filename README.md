# Beacon

A modern, open-source Minecraft launcher inspired by Prism Launcher — but with a
cleaner, faster UI. Manage isolated instances, install mods and modpacks from
Modrinth, sign in with your Microsoft account, and launch the game with the
right Java version automatically.

> Built with **Tauri 2** (Rust backend) + **React + TypeScript + Tailwind CSS**.

---

## Features

- **Instances** — create, duplicate and delete isolated Minecraft installations.
  Each instance has its own mods, config, saves and options.
- **Mod loaders** — Vanilla, **Fabric** and **Quilt** are fully supported
  (Forge / NeoForge are planned — see _Roadmap_).
- **Mods** — search and install from **Modrinth** straight into an instance,
  with enable/disable and removal.
- **Accounts** — real **Microsoft / Minecraft** sign-in via the OAuth
  device-code flow, plus **offline** accounts for testing.
- **Java** — detects installed JDK/JREs and **auto-downloads** the correct
  Temurin runtime per Minecraft version when needed.
- **Smart downloads** — parallel, resumable, SHA-1 verified; shared asset and
  library cache across instances (just like Prism/MultiMC).
- **Modern UI** — custom frameless window, light/dark themes, accent colors,
  smooth animations, live download progress and a streaming game log console.

---

## Prerequisites

- **Windows 10/11** (this MVP targets Windows; the code is largely
  cross-platform).
- [**Node.js**](https://nodejs.org/) 20+ and npm
- [**Rust**](https://rustup.rs/) (stable, MSVC toolchain)
- **Visual Studio C++ Build Tools** (MSVC + Windows SDK) — required to link the
  Rust backend on Windows.
- **WebView2 Runtime** — preinstalled on Windows 11.

---

## Getting started

```bash
# 1. Install JS dependencies
npm install

# 2. Run in development (opens the app with hot-reload)
npm run tauri dev

# 3. Build a distributable (installer + portable exe)
npm run tauri build
```

The first `tauri dev` / `cargo build` compiles all Rust dependencies and can
take several minutes. Subsequent builds are incremental and fast.

---

## Microsoft login setup (one-time, ~2 minutes)

To sign in with a real Minecraft account, Beacon needs an **Azure application
(public client) ID**. Each user supplies their own — this keeps the launcher
compliant with Microsoft's terms (everyone authenticates with their own valid
account).

1. Go to the [Azure Portal → App registrations](https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps/ApplicationsListBlade)
   and click **New registration**.
2. Name it anything (e.g. "Beacon"). Under **Supported account types** choose
   **Personal Microsoft accounts**.
3. Leave the redirect URI empty and register.
4. Open **Authentication → Advanced settings** and set **Allow public client
   flows** to **Yes**.
5. Copy the **Application (client) ID** from the Overview page.
6. Set it as an environment variable before launching Beacon:

   ```powershell
   $env:BEACON_CLIENT_ID = "<your-application-client-id>"
   npm run tauri dev
   ```

   (For a production build, bake it into your environment or a `.env`/launch
   script.)

Without a client ID you can still use **Offline accounts** to create instances,
install mods and explore the launcher — you just can't play online.

---

## Where data lives

Everything is stored under the OS app-data directory:

```
%APPDATA%\com.beacon.launcher\
├── instances\<id>\
│   ├── instance.json        # instance metadata
│   └── minecraft\           # the game directory (mods, saves, config, ...)
├── libraries\               # shared maven libraries
├── assets\                  # shared asset objects + indexes
├── versions\                # version JSON + client jars
├── java\                    # auto-downloaded Java runtimes
├── natives\                 # extracted native libs (per launch)
├── settings.json
└── accounts.json
```

---

## Project structure

```
beacon-launcher/
├── src/                     # React + TypeScript frontend
│   ├── components/          # UI primitives, title bar, sidebar, modals, cards
│   ├── pages/               # Instances, instance detail, accounts, settings
│   ├── store/               # Zustand state + event wiring
│   └── lib/                 # api bridge, types, utilities
└── src-tauri/               # Rust backend
    └── src/
        ├── mojang.rs        # version manifest, libraries, assets, inheritance
        ├── modloader.rs     # Fabric / Quilt profiles
        ├── modrinth.rs      # mod search + install
        ├── auth.rs          # Microsoft device-code + Xbox/XSTS exchange
        ├── java.rs          # Java detection + Adoptium download
        ├── launch.rs        # classpath/arg assembly + process management
        ├── instances.rs     # instance/settings/account persistence
        ├── net.rs           # parallel downloads + SHA verification
        └── commands.rs      # Tauri command surface
```

---

## Roadmap

- Forge / NeoForge launching (installer + processor support)
- Modpack installation from Modrinth (`.mrpack`)
- Resource packs, shader packs and world management
- Instance import/export
- Self-update

---

## Legal

Beacon uses only official, public APIs (Microsoft OAuth, Mojang/`piston-meta`,
Modrinth, Adoptium) and requires a valid Minecraft account to play online.
"Minecraft" is a trademark of Mojang Synergies AB; Beacon is not affiliated with
or endorsed by Mojang or Microsoft.
