# Updater key recovery runbook / Updater 密钥恢复手册

The committed `src-tauri/updater.pubkey` is development-only. Its private key
was destroyed after generation, and `scripts/check-release-identity.mjs`
rejects its fingerprint. It can exercise development configuration but cannot
produce a public release.

仓库中的 `src-tauri/updater.pubkey` 仅用于开发。生成后私钥已销毁，
`scripts/check-release-identity.mjs` 会阻止其 fingerprint 用于正式发布。它
可以验证开发配置，但无法生成公开 Release。

Before the first public release:

1. Generate a password-protected Tauri updater key on an offline trusted device.
2. Commit only the public key and its SHA-256 fingerprint; replace the key in
   `tauri.conf.json`.
3. Store the private key/password in the protected `preview-release` GitHub
   Environment, never in repository variables, workflow artifacts, logs, or
   fork pull requests.
4. Create two independently encrypted offline recovery copies in separate
   physical or administrative locations.
5. Restore each copy in an isolated temporary environment, sign a test file,
   verify it with the committed public key, then securely erase the temporary
   plaintext.
6. Record the drill date and maintainers privately. Only then replace the
   development fingerprint blocker in the release check.

首次公开发布前：

1. 在离线可信设备生成带密码保护的 Tauri updater key。
2. 只提交公钥和 SHA-256 fingerprint，并替换 `tauri.conf.json` 中的 key。
3. 私钥/密码只存入受保护的 `preview-release` GitHub Environment，不得进入
   仓库变量、workflow artifact、日志或 fork pull request。
4. 制作两份独立加密的离线恢复副本，存放在不同物理或管理位置。
5. 分别在隔离临时环境恢复、签署测试文件并用仓库公钥验证，随后安全清除临时
   明文。
6. 私下记录演练日期和维护者。只有完成后才能替换发布检查中的开发 fingerprint
   阻断。

Key rotation requires a bridge release signed by the old key that embeds the
new public key. Losing the old private key cannot be repaired for already
installed clients by merely generating a new pair.

轮换必须先由旧 key 签发一个内嵌新公钥的过渡版本。旧私钥丢失后，仅重新生成
一对 key 不能恢复已安装客户端的自动更新链路。
