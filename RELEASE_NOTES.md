## EZMapa 0.3.6 - Developer Hub

This release adds a private workspace for Minecraft project development and
polishes the launcher's activity experience.

### Private Developer Hub

- Discover supported Minecraft projects from local development folders.
- Inspect project type, version, loader, Git status, and build output in one
  launcher view.
- Run supported development builds without leaving EZMapa.
- Copy built artifacts and install them directly into a selected instance.
- Keep development tools private and local; project discovery and build actions
  do not introduce a background service.

### Activity and reliability

- Render the Activity Center at the app root so it stays above transformed and
  translucent interface layers.
- Close the Activity Center with Escape and expose its expanded state to
  assistive technology.
- Refresh compatible frontend and Rust dependencies.
- Resolve new strict-Clippy findings in project sorting and network tests.

Existing installations will receive this release through the signed in-app
updater.
