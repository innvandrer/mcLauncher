# EZMapa

A modern Minecraft launcher for the EZMapa community. Manage isolated instances,
install mods and modpacks from **Modrinth** and **CurseForge**, sign in with your
Microsoft account, and launch the game with the right Java version automatically.
EZMapa **updates itself** in the background, so you always run the latest version.

> Built with **Tauri 2** (Rust backend) + **React + TypeScript + Tailwind CSS**.

---

## Features

- **Instances** — create, duplicate, import, export and delete isolated Minecraft
  installations. Each instance has its own mods, config, saves and options, and
  can be organised into groups.
- **Mod loaders** — Vanilla, **Fabric**, **Quilt**, **Forge** and **NeoForge** are fully supported.
- **Content from two providers** — search and install **mods, modpacks, resource
  packs and shaders** from **Modrinth** _and_ **CurseForge**, straight into an
  instance. Browse with pagination, see which items are already installed, and
  pick a specific version.
- **Modpacks** — one-click install from Modrinth (`.mrpack`) or CurseForge; each
  pack creates a new, ready-to-play instance.
- **Mod updates** — check installed mods against the provider and apply updates
  in place.
- **Content management** — enable/disable/remove mods, manage resource packs,
  shaders, worlds and screenshots per instance.
- **Accounts** — real **Microsoft / Minecraft** sign-in directly in the app
  (no setup required), plus **offline** accounts for testing. Your player skin
  shows up in the sidebar and accounts list.
- **Java** — detects installed JDK/JREs and **auto-downloads** the correct
  Temurin runtime per Minecraft version when needed.
- **Smart downloads** — parallel, resumable, SHA-1 verified; shared asset and
  library cache across instances (just like Prism/MultiMC).
- **Self-update** — checks for new releases on launch and updates itself with a
  one-click prompt (signed updates verified against an embedded public key).
- **Modern UI** — custom frameless window, light/dark themes, accent colors,
  smooth animations, live download progress and a streaming game log console.

---

## Prerequisites (for building from source)

- **Windows 10/11** (this MVP targets Windows; the code is largely
  cross-platform).
- [**Node.js**](https://nodejs.org/) 20+ and npm
- [**Rust**](https://rustup.rs/) (stable, MSVC toolchain)
- **Visual Studio C++ Build Tools** (MSVC + Windows SDK) — required to link the
  Rust backend on Windows.
- **WebView2 Runtime** — preinstalled on Windows 11.

> Just want to use EZMapa? Grab the latest installer from the
> [**Releases**](https://github.com/innvandrer/mcLauncher/releases) page and run
> it — the app keeps itself up to date after that.

---

## Getting started

```bash
# 1. Install JS dependencies
npm install

# 2. Run in development (opens the app with hot-reload)
npm run tauri:dev

# 3. Build a distributable installer
npm run tauri:build
```

The first `tauri dev` / `cargo build` compiles all Rust dependencies and can
take several minutes. Subsequent builds are incremental and fast.

---

## Signing in

Click **Add Microsoft account** on the Accounts page and complete the login in
the window that opens — no configuration needed. EZMapa requires a valid
Minecraft account to play online.

Prefer not to sign in? **Offline accounts** let you create instances, install
mods and explore the launcher (singleplayer / testing only).

---

## CurseForge setup (optional)

Modrinth works out of the box. To also search and install from **CurseForge**,
you need a free CurseForge **Core API key**:

- Set it in **Settings → Content providers**, or
- Provide it via the `EZMAPA_CF_API_KEY` environment variable before launching.

Without a key, Modrinth remains fully available.

---

## Releasing & self-update

EZMapa ships signed updater artifacts and checks
`https://github.com/innvandrer/mcLauncher/releases/latest/download/latest.json`
on launch. To cut a new release:

1. Bump `version` in `package.json` **and** `src-tauri/tauri.conf.json`.
2. Build with the signing key available:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content <path-to>\ezmapa.key -Raw
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<key password>"
   npm run tauri:build
   ```

3. Generate `latest.json` (signature = contents of the `*-setup.exe.sig`, url =
   the versioned release asset URL) and publish a GitHub release with the
   installer + `latest.json` attached:

   ```bash
   gh release create vX.Y.Z <…>-setup.exe latest.json --repo innvandrer/mcLauncher
   ```

Installed copies on an older version will then prompt to update on next launch.
Keep the private key **and** its password safe — without them you can't sign
updates that existing installs will accept.

---

## Where data lives

Everything is stored under the OS app-data directory:

```
%APPDATA%\com.ezmapa.launcher\
├── instances\<id>\
│   ├── instance.json        # instance metadata
│   ├── ezmapa_index.json    # installed-content index (file → project/provider)
│   └── minecraft\           # the game directory (mods, saves, config, ...)
├── libraries\               # shared maven libraries
├── assets\                  # shared asset objects + indexes
├── versions\                # version JSON + client jars
├── java\                    # auto-downloaded Java runtimes
├── natives\                 # extracted native libs (per launch)
├── settings.json
└── accounts.json
```

If you previously used the Beacon build, your data is migrated automatically from
`%APPDATA%\com.beacon.launcher\` on first launch.

---

## Project structure

```
ezmapa-launcher/
├── src/                     # React + TypeScript frontend
│   ├── components/          # UI primitives, title bar, sidebar, modals, cards
│   ├── pages/               # Instances, instance detail, modpacks, accounts, settings
│   ├── store/               # Zustand state + event wiring
│   └── lib/                 # api bridge, types, utilities
└── src-tauri/               # Rust backend
    └── src/
        ├── mojang.rs        # version manifest, libraries, assets, inheritance
        ├── modloader.rs     # Fabric / Quilt profiles
        ├── modrinth.rs      # Modrinth search + install + .mrpack modpacks
        ├── curseforge.rs    # CurseForge search + install + modpacks
        ├── auth.rs          # Microsoft login + Xbox/XSTS/Minecraft token chain
        ├── java.rs          # Java detection + Temurin download
        ├── launch.rs        # classpath/arg assembly + process management
        ├── instances.rs     # instance/settings/account persistence + import/export
        ├── net.rs           # parallel downloads + SHA verification
        └── commands.rs      # Tauri command surface
```

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the current Now / Next / Later plan.

---

## Legal

EZMapa uses official, public APIs (Microsoft sign-in, Mojang/`piston-meta`,
Modrinth, CurseForge, Adoptium) and requires a valid Minecraft account to play
online. "Minecraft" is a trademark of Mojang Synergies AB; EZMapa is not
affiliated with or endorsed by Mojang or Microsoft.
