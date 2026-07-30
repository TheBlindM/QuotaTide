# Verify a release

Download only from the project's bound GitHub Releases page. Each draft/final
release contains `SHA256SUMS`, `latest.json`, installers, updater artifacts, and
their Tauri signatures.

On macOS:

```sh
shasum -a 256 QuotaTide_<version>_universal.dmg
grep "QuotaTide_<version>_universal.dmg" SHA256SUMS
```

On Windows PowerShell:

```powershell
Get-FileHash .\QuotaTide_<version>_x64-setup.exe -Algorithm SHA256
Select-String -Path .\SHA256SUMS -Pattern "QuotaTide_<version>_x64-setup.exe"
```

The values must match exactly. GitHub build provenance is generated for final
release bytes; verify it with GitHub's artifact-attestation instructions once
the repository is bound. `latest.json` must contain only
`darwin-aarch64`, `darwin-x86_64`, and `windows-x86_64`; both macOS entries
must point to the same universal archive. A `.sig` value is signature content,
not a file path or URL.

Unsigned preview means the operating system cannot verify a publisher identity.
It does not mean checksum or updater signature verification is optional.
