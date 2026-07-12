## EZMapa 0.3.1

Fixes Forge/NeoForge installs failing with an "AccessDenied" error on some Windows setups.

### Fixes
- **Forge/NeoForge install failure** — the loader installer staged its work in the system temp directory, which could resolve to a protected location (e.g. `C:\Windows\Temp` when the launcher runs elevated). The installer's final jar-splitting step could not write there and failed with `java.nio.file.AccessDeniedException`. Staging now happens under the launcher's own data folder, which is always writable regardless of how EZMapa is launched.
