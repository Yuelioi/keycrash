# 开发与构建

版本号由根目录 `Cargo.toml` 的 `package.version` 管理，自动显示在窗口标题和界面品牌名右侧。

```powershell
cargo run --target-dir (./scripts/get-target-root.ps1)
cargo test --locked --workspace --target-dir (./scripts/get-target-root.ps1)
./scripts/build-release.ps1
```

构建和 fixture 测试脚本读取 Cargo 的 target 根目录（优先使用全局 `CARGO_TARGET_DIR`），再拼接 `keycrash`，release 产物位于 `<根目录>\keycrash\release`。迁移时只需修改全局变量，脚本不写死盘符，也不修改全局变量。未设置变量时遵循 Cargo 配置或默认目录。测试脚本也可通过 `-ReleaseRoot` 指定产物目录。

GitHub Actions 配置位于 `.github/workflows/build.yml`，在 push、PR 或手动触发时执行 Windows 测试、release 构建和图标检查，并上传便携包（保留 14 天），不自动创建 GitHub Release。

发布前需要明确选择 Slint 的 GPLv3、Royalty-free attribution 或商业许可路径。
