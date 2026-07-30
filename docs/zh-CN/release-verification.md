# 校验发布版本

只从项目绑定后的 GitHub Releases 页面下载。每个 draft/正式 Release 都包含
`SHA256SUMS`、`latest.json`、安装包、updater 产物及 Tauri 签名。

macOS：

```sh
shasum -a 256 QuotaTide_<version>_universal.dmg
grep "QuotaTide_<version>_universal.dmg" SHA256SUMS
```

Windows PowerShell：

```powershell
Get-FileHash .\QuotaTide_<version>_x64-setup.exe -Algorithm SHA256
Select-String -Path .\SHA256SUMS -Pattern "QuotaTide_<version>_x64-setup.exe"
```

两边的值必须完全一致。最终发布字节会生成 GitHub build provenance；仓库绑定
后可按 GitHub artifact attestation 文档校验。`latest.json` 只能包含
`darwin-aarch64`、`darwin-x86_64`、`windows-x86_64`，两个 macOS 条目必须
指向同一个 universal archive。`.sig` 字段是签名内容，不是文件路径或 URL。

“未签名预览版”表示操作系统无法验证发布者身份，并不表示可以跳过 checksum
或 updater 签名验证。
