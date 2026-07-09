## EZMapa 0.2.5 (prerelease)

Security hardening and instance export. This build is tagged as a GitHub prerelease so it won't auto-update existing installs — please test manually before it's promoted.

### Security
- **Account tokens moved to the OS keyring** (Windows Credential Manager / macOS Keychain / Linux Secret Service) instead of plaintext in `accounts.json`. Existing installs migrate automatically on first launch.
- **Zip-slip protection** when extracting modpacks/archives — entries that try to escape the target folder via `..` or absolute paths are rejected.
- **URL scheme validation** before opening links in the system browser — only `http://`/`https://` allowed.

### What's new
- **Export instances** as a full `.zip` backup or a shareable `.mrpack` modpack, from the instance card, detail page, or Command Palette (⌘K).
- Account actions (sign-in, switch, remove) now surface errors as toasts instead of failing silently.
- CI now runs the frontend build and Rust test suite on every push/PR.

## EZMapa 0.2.4

Content tab improvements across every instance sub-tab.

### What's new
- **Paginated installed lists** — mods, resource packs, and shaders now paginate long installed lists (20 per page), matching the browse panel.
- **Version picker for packs & shaders** — resource packs and shaders now open the version picker on install, like mods.
- **Worlds & screenshots** — search filter plus pagination when you have many worlds or screenshots.
