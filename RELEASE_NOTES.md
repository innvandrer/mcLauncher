## EZMapa 0.2.6

Fixes Microsoft login sessions failing on multiplayer servers, and enables automatic launcher updates on startup.

### Fixes
- **Invalid session on servers** — Microsoft tokens now persist in Windows Credential Manager (the `keyring` crate was missing the `windows-native` feature, so tokens were stored in memory only and lost between launches).
- **Safer session handling** — the launcher refreshes missing tokens before launch and refuses to start the game with an empty session instead of passing a dummy token.

### Changes
- **Automatic launcher updates** — when a newer release is available, EZMapa downloads and installs it on startup, then restarts. If auto-update fails, the manual update prompt is shown instead.
