# Code Review — EZMapa Launcher (v0.3.1, `main` @ 9ac79b1)

Scope: full pass over the Rust backend (~13.7k lines, 34 modules), the React/TypeScript
frontend, Tauri configuration, and CI/release workflows. The frontend typechecks
(`tsc`) and builds (`vite`) cleanly; the Rust suite could not be compiled in this
review environment (no webkit2gtk on the container — CI covers it on `windows-latest`).

## Overall assessment

This is a well-built codebase — noticeably above hobby-launcher quality. Highlights
worth calling out before the findings:

- Clean layering: thin `commands.rs` → domain modules → shared `net`/`state` plumbing.
- Security consciousness in the right places: OS-keyring token storage with read-back
  verification and a legacy-migration path (`account_tokens.rs`, `instances.rs`),
  zip-slip guards with tests (`archive.rs`, `enclosed_name()`), DOMPurify on
  CurseForge HTML, a real CSP, a minisign-signed updater, checksum-verified downloads.
- Non-trivial protocol work implemented from spec **with reference-vector tests**:
  murmur2 fingerprinting, NBT parsing, Server List Ping, FML packed-UTF16 mod lists.
- Honest engineering docs (module headers explain *why*; `server_mods.rs` even
  documents what has *not* been validated), a maintained ROADMAP, clippy `-D warnings`
  in CI, ~60 unit tests focused on the pure logic.

The findings below are ordered by how much I'd prioritize them.

---

## Correctness

### 1. `Cargo.toml`: every core dependency is desktop-gated (latent build break)

`src-tauri/Cargo.toml:25` opens
`[target."cfg(not(any(target_os = \"android\", target_os = \"ios\")))".dependencies]`
for `tauri-plugin-updater` — but a TOML table extends until the next header, so
**everything below it** (`serde`, `tokio`, `reqwest`, `zip`, `keyring`, `chrono`,
`uuid`, `thiserror`, …) is also desktop-only. Verified via `cargo metadata`: 18 of 22
dependencies carry the target gate. It works today because only desktop is built, but
`lib.rs` carries `#[cfg_attr(mobile, tauri::mobile_entry_point)]` and any future
mobile (or workspace) build will fail confusingly. Fix: move the updater line to the
**end** of the file, or move the general `[dependencies]` block above it.

### 2. Blocking work on the async runtime

Several `async` paths run long blocking operations directly on tokio workers. The
codebase clearly knows the right pattern (`spawn_blocking` is used for Java detection
in `commands.rs:1152`, natives extraction in `mojang.rs:666`, unzip in `java.rs:213`),
but these spots miss it:

- `forge.rs:249` — `cmd.output()` runs the Forge/NeoForge installer **synchronously
  for minutes** ("this may take a minute…") inside `async fn run_installer`.
- `launch.rs:365` — processor `cmd.status()` inside `async fn launch` (see #5).
- `launch.rs:463` — the pre-launch hook (`run_shell`) blocks the async `launch`
  command for as long as the user's command runs.
- `curseforge.rs:772-831`, `modrinth.rs:627-684`, `instances.rs:539-579` — zip
  parsing/extraction of whole modpacks inline in async fns.
- `tools.rs:121-216` — snapshot create/restore (zipping a multi-GB world) are called
  from async commands.

Each of these ties up a tokio worker thread; a couple of concurrent heavy operations
can stall unrelated UI commands. Wrap them in `tokio::task::spawn_blocking` (or use
`tokio::process::Command` — the `process` feature is already enabled).

### 3. Resource-pack/shader updates via Modrinth never match on modded instances

`modrinth.rs:1038-1043` (`check_updates`) sends **one** `version_files/update`
request for all collected hashes — mods *and* resource packs *and* shaders — with
`loaders: [<instance loader>]` (the frontend always passes it,
`InstanceDetailPage.tsx:571-575`). Resource-pack/shader versions on Modrinth are
tagged `minecraft`/`iris`/`optifine`, never `fabric`/`forge` — your own comment in
`ensure_complementary_shader` (`modrinth.rs:330-332`) states exactly this. So on any
modded instance, pack/shader updates from Modrinth are silently never offered (the
CurseForge path handles it correctly — `pick_file_index` treats `mod_loader: None`
as match-any, `curseforge.rs:1089-1103`). Fix: query mods with the loader filter and
packs/shaders without it (two batches), mirroring the CF behavior.

### 4. Adoptium auto-download assumes `.zip` — breaks on Linux/macOS

`java.rs:190-222` downloads
`api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/...` and always feeds it
to `unzip()`. Adoptium serves **.tar.gz for Linux and macOS** (zip is Windows-only),
so auto-provisioning a JRE fails off-Windows. Latent today (README: Windows MVP), but
the ROADMAP lists cross-platform builds, and everything else in this file is
carefully cfg-gated per OS — this is the piece that will actually bite. Also worth
noting: the JRE (~40 MB) is buffered fully in memory and not checksum-verified
(Adoptium's API exposes checksums; see #9).

### 5. The Forge "processor" launch path appears to be dead code — and wrong if it ever runs

`mojang.rs:183-193` deserializes a `processors` field from version JSONs and
`launch.rs:335-370` executes them on **every launch**. But Forge/NeoForge processors
live in `install_profile.json`, not the version JSON, and `run_installer` only copies
the version JSON out of staging (`forge.rs:271-276`) after the official installer has
already executed the processors itself. So `resolved.processors` should always be
empty. If it ever weren't, this code would still misbehave: Forge processor args use
`{BINPATCH}`-style placeholders while `substitute()` only replaces `${...}`
(`launch.rs:19-25`), and install-time processors shouldn't re-run per launch. This
block is also where the Norwegian comments/log lines live ("Kjører Forge-prosessorer",
"Steg 2" — `launch.rs:335-341`, `mojang.rs:183,213-222`), which reach users in an
otherwise all-English app. Recommend deleting the path (or documenting why it exists)
— it's beyond the `#[allow(dead_code)]` cleanup already on the ROADMAP.

### 6. Two disagreeing "required Java for MC version" tables

`preflight.rs:50-67` says MC 1.17 → Java **16**; `jvmtune.rs:66-84` says 1.17 → Java
**17**. One of them is duplicated, and one is wrong (Mojang's launcher provisions 16
for 1.17). Consolidate into one function (preflight's, which also handles snapshots
and `-rc1` suffixes) and reuse it from both modules — `java::ensure`'s "any newer
major" fallback (`java.rs:178-183`) makes this table the only guardrail against
launching old MC on a too-new JVM.

### 7. Expired Microsoft session with no refresh token launches anyway

`launch.rs:210-226`: `needs_refresh` covers empty-or-expired, but when
`refresh_token` is `None` the code falls through and only errors if the access token
is **empty** — an expired-but-present token proceeds to launch, and the user finds
out via in-game server auth failures instead of the launcher's clear "sign in again"
error. Treat `expired && no refresh token` the same as the empty case.

---

## Security hardening

Context: the webview renders remote, author-controlled content (mod descriptions).
You've mitigated the front line well — CSP with `script-src 'self'`
(`tauri.conf.json`), DOMPurify for CF HTML, ReactMarkdown (no raw HTML) for Modrinth.
The items below are the second line of defense, for the day a sanitizer bypass or
webview bug shows up.

### 8. Path-type command arguments are trusted verbatim

Many commands join webview-supplied strings straight into filesystem paths:

- `delete_instance(id)` → `remove_dir_all(instances/<id>)` (`instances.rs:397-403`) —
  `id = ".."` deletes the whole data root.
- `delete_world(name)` → `remove_dir_all(saves/<name>)` (`instances.rs:968-974`);
  same shape in `delete_mod`, `delete_resource_pack`, `delete_shader`,
  `set_mod_enabled`, `restore_snapshot`, `get_instance`, `open_*` commands.
- `save_png(dest, data)` (`commands.rs:1236`) is an arbitrary-path write primitive
  (suffix check only); `set_skin_file`/`save_skin_file` read arbitrary paths;
  `upload_log(content)` posts arbitrary text to mclo.gs.

You already built exactly the right tools for this — `archive::safe_join` for zip
entries and `validate_http_url` for `open_url` — the same rigor just isn't applied to
command inputs. A small helper (reject `id`/`file_name`/`name` containing separators
or `..`, e.g. require a single `Component::Normal`) applied at the top of each command
closes the whole class cheaply. The dialog-driven paths (`save_png`, skin import,
export destinations) are fine to leave as user-chosen, but worth noting they're
webview-invokable with arbitrary arguments.

Related nit: `capabilities/default.json` ships
`core:webview:allow-internal-toggle-devtools` in production.

### 9. Executables are downloaded and run without checksum pinning

`forge.rs:197-250` downloads the Forge/NeoForge installer JAR and executes it;
`java.rs:190-222` downloads and unpacks a whole JRE. Both trust TLS alone. Maven
repos publish `<artifact>.sha1` next to every file, and Adoptium's API serves
checksums — verifying before execution (like every game file download already does
via `DownloadItem.sha1`) would close the gap and also catch truncated downloads.

### 10. Modpack archives still go to the system temp dir

v0.3.1's whole point was moving installer staging out of `std::env::temp_dir()`
because protected temp locations break elevated runs — but pack downloads still use
it: `curseforge.rs:768` (`ezmapa_cf_{file_id}.zip`) and `modrinth.rs:614`
(`fetch_mrpack_archive`). Same failure class, plus predictable names in a shared,
world-writable directory. Route them through `state.dirs.cache()` like the installer.

---

## Minor

- **`net.rs:74`** — `path.with_extension("part")` *replaces* the extension, so
  `foo.jar` and `foo.zip` in one directory share the temp name `foo.part`.
  `instances::atomic_write` (`instances.rs:30-33`) already documents why appending
  (`foo.jar.part`) is the right pattern; `net.rs` contradicts it.
- **`instances.rs:472-493`** — `record_session_dirs` is an unlocked read-modify-write
  of `sessions.json` from per-game watcher threads; two games exiting simultaneously
  can lose a session. (Same for `record_play_dirs`, but that's per-instance-file.)
- **`instances.rs:574-576`** — comment says "reset runtime stats" but only
  `last_played` is reset; `total_play_seconds` is imported along with the zip.
- **Zip writers buffer whole files** — `zip_dir_into` (`instances.rs:515`),
  `zip_dir_recursive` (`tools.rs:166`), `write_zip` (`export.rs:261`) all
  `fs::read` entire files; a multi-GB world snapshot spikes memory. `std::io::copy`
  from a `File` into the `ZipWriter` streams.
- **Duplication** — three `copy_dir` implementations (`lib.rs:53`, `forge.rs:307`,
  `instances.rs:420`), two hand-rolled base64 codecs (`skin.rs:171,264`), two
  percent-encoders (`auth.rs:37`, `server_mods.rs:633`), two required-Java tables
  (#6). Worth one `util` pass; the base64 pair could just be the `base64` crate.
- **`useStore.ts:308-337`** — `checkForUpdates` silently `downloadAndInstall()`s and
  `relaunch()`es on startup; the consent modal only appears if the silent path fails.
  README says self-update is by design, but an unprompted relaunch while the user is
  mid-edit is worth a beat of UX thought (install on next start, or always prompt).
- **`launch.rs:583-601`** — `stop()` on Unix sends SIGTERM to the JVM only; Windows
  uses `/T` (tree). Fine today (Windows-only), noted for the cross-platform milestone.
- **`InstanceDetailPage.tsx`** — 2,759 lines / ~15 components in one file. It's
  internally well-decomposed, so splitting it is mechanical — worth doing before it
  grows further.
- **`discord.rs:19-24`** — `DiscordPresence::new()` does a synchronous IPC connect
  during app setup; harmless when Discord is absent (fails fast) but it's on the
  startup path.
- **`Settings.curseforge_api_key`** is plaintext in `settings.json` — acceptable for
  a user-supplied API key, just worth a line in the docs.

## Things the ROADMAP already tracks (agreed, no action needed here)

README's "resumable downloads" overclaim, the `#[allow(dead_code)]` cleanup,
an automatic backup before `delete_world`, ESLint/`cargo fmt` in CI, i18n extraction.

## Suggested order of attack

1. #1 (one-line Cargo.toml move) and #6/#7 (small, user-visible correctness).
2. #3 (update checker) — silent feature failure, easy two-batch fix.
3. #8 (input validation helper) + #10 (temp dir) — cheap hardening.
4. #2 (spawn_blocking sweep) and #5 (delete the processor path) together — both live
   in the launch/install path and are easiest to verify in one pass.
5. #9 and #4 when cross-platform work starts.
