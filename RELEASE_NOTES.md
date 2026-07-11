## EZMapa 0.3.0

Five major features around one theme: EZMapa now understands that the same mod lives on both Modrinth and CurseForge, and uses that everywhere — blocked downloads, updates, exports, servers, and performance.

### New
- **Blocked CurseForge downloads auto-resolve via Modrinth** — when a mod author has disabled third-party downloads, EZMapa finds the byte-identical file on Modrinth (verified against CurseForge's own hash) and installs it from there. Modpack installs no longer fail on blocked files: anything unresolvable is listed at the end with direct CurseForge page links, and the progress card shows how many files were auto-resolved.
- **Update checking across both platforms** — every installed mod, resource pack, and shader is checked against Modrinth *and* CurseForge, whichever it was installed from. The newest compatible release wins (by release date, never version-string guessing), each proposed update shows a source badge, and a per-mod "switch source of truth" action re-pins a mod to the other platform.
- **CurseForge modpack export + "Export both"** — instances can now export as a CurseForge pack (manifest.json + modlist.html + overrides) alongside `.mrpack`, from one shared export pipeline. Mods that only exist on the other platform go through a review dialog: excluded by default, embeddable per mod (or "embed all") after a license warning.
- **Create matching instance from a server** — modded (Forge/NeoForge) servers in the server browser can be turned into a ready instance: EZMapa decodes the mod list from the server's ping response (including the compressed modern format), matches each mod to Modrinth/CurseForge, installs everything it can, and reports the rest with search links. Best effort: server-only mods are skipped and client-only mods can't be detected.
- **Recommended JVM settings per instance** — one click suggests a heap size (from pack size, capped against your RAM) and GC flags matched to your Java version: generational ZGC on Java 21+, Aikar-style G1 on 17–20. Shown as a before/after diff — custom arguments are merged, never overwritten silently. Startup time is now measured per session, so after a change the instance page shows "avg startup before/after".

### Changes
- Mods installed from CurseForge modpacks are now tracked with their project identity (enables updates and exports for pack mods).
- The launcher no longer guesses CDN links for files the CurseForge API refuses to serve — the only automatic fallback is the hash-verified identical file from Modrinth.
