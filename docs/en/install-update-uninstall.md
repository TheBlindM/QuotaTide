# Install, update, and uninstall

## macOS 15+

Download the universal DMG from the project's GitHub Release and verify it
before opening. Drag QuotaTide to Applications. This unsigned preview has no
Developer ID or notarization, so Gatekeeper may block the first launch.

Use only Apple's scoped UI exception: Control-click QuotaTide in Finder and
choose **Open**, or use the app-specific **Open Anyway** control in System
Settings when it appears. QuotaTide will never ask you to disable Gatekeeper
globally.

To uninstall, quit QuotaTide and move it from Applications to Trash. To remove
its settings, history, alert outbox, and credential-vault entries first, use
**Settings → Privacy → Remove local data**. That action does not remove or edit
`auth.json`.

## Windows 11 25H2+ x64

Download the x64 `setup.exe` from the project's GitHub Release and verify it.
The NSIS installer installs for the current user under `%LOCALAPPDATA%`; it
does not need administrator access. It uses the Evergreen WebView2 bootstrapper
if the runtime is absent.

Because the preview has no Authenticode certificate, SmartScreen may show
**Unknown publisher**. Proceed only after confirming the GitHub source and
SHA-256. QuotaTide will never ask you to turn off SmartScreen, Smart App
Control, antivirus, or certificate validation globally.

Uninstall from **Settings → Apps → Installed apps → QuotaTide**. Use the
in-app local-data removal control first if you also want to remove settings,
history, and credential-vault entries. `auth.json` remains untouched.

## Updates

QuotaTide waits 60 seconds after startup before its first automatic check and
then checks at most once every 24 hours. Automatic checking can be disabled;
manual checking remains available under **Account → About & Updates**.

An available update shows its version and notes. Nothing downloads or installs
until you choose **Install and restart** and confirm. The current version
remains installed if the network, URL, signature, disk write, or installer
fails. Tauri updater signatures are mandatory even though OS publisher signing
is not currently available.
