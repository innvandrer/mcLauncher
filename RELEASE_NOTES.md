## EZMapa 0.2.3

Security, reliability, and performance hardening.

### Security
- CurseForge mod/modpack descriptions are now sanitized before they're shown, and a strict Content-Security-Policy is enforced app-wide — closing a cross-site-scripting hole where a malicious project page could run code in the launcher.

### Reliability
- Settings, accounts, and play stats are now saved atomically, so a crash or power loss mid-write can no longer corrupt them.
- Fixed a race where signing in or switching accounts while another action was running could lose changes.

### Performance
- Faster launches: the Minecraft version manifest is cached and Java detection runs once, off the UI thread.
- Lower memory use when checking for mod updates or exporting large modpacks (files are now hashed in a stream instead of being read whole).
- Smoother log output for heavily-modded instances.

### Stuck "updating to 0.2.2" over and over?
That loop is a one-time side effect of the **Beacon → EZMapa** rename: the EZMapa installer can't replace the old **Beacon** app, so it installs alongside it. To fix it once: uninstall **"Beacon"** in Windows **Settings → Apps**, then launch **"EZMapa"** (reinstall it if needed). Updates apply normally after that.
