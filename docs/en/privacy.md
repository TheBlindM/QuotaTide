# Privacy

QuotaTide is local-first and has no project telemetry or analytics service.

It reads only the user-selected `auth.json` to request the current Codex quota.
It does not modify that file, copy its tokens into the application database, or
send the token to QuotaTide maintainers. Quota refresh contacts the same
upstream Codex usage service represented by the selected account.

Reset activity contacts `https://www.codexrunway.com/api/status.json`. Update checks contact
the configured GitHub Release endpoint. These requests expose ordinary network
metadata such as IP address and user agent to those services, but QuotaTide
does not add the Codex token, account ID, quota history, email address, or a
device identifier to reset-radar or updater requests.

Quota observations, settings, alert history, and source health are stored in a
local SQLite database. SMTP passwords are stored in macOS Keychain or Windows
Credential Manager; other SMTP settings and recipient addresses are local
settings. Email alerts send the configured alert content to the chosen SMTP
server and recipients.

The app can export a redacted diagnostic ZIP and can remove its own local data.
It never deletes or modifies `auth.json`. See the privacy screen before export
or deletion for the exact current paths.
