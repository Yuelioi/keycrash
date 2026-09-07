# 开发与构建

版本号由根目录 `Cargo.toml` 的 `package.version` 管理，自动显示在窗口标题和界面品牌名右侧。

```powershell
cargo run --target-dir (./scripts/get-target-root.ps1)
cargo test --locked --workspace --target-dir (./scripts/get-target-root.ps1)
./scripts/build-release.ps1
```

构建和 fixture 测试脚本读取 Cargo 的 target 根目录（优先使用全局 `CARGO_TARGET_DIR`），再拼接 `keycrash`，release 产物位于 `<根目录>\keycrash\release`。迁移时只需修改全局变量，脚本不写死盘符，也不修改全局变量。未设置变量时遵循 Cargo 配置或默认目录。测试脚本也可通过 `-ReleaseRoot` 指定产物目录。

GitHub Actions 分为两条流程：

- `.github/workflows/build.yml`：代码、资源、构建配置变更的分支 push / PR 执行测试与编译检查，不打包、不上传产物、不发布。纯文档提交不触发。
- `.github/workflows/release.yml`：仅推送 `v*` tag 时执行。先验证 tag 与根目录 `Cargo.toml` 版本完全一致，再测试、构建、打包并创建 GitHub Release，附带 Windows ZIP。含 `-` 的预发布版本标记为 prerelease。

发布顺序：修改版本号并更新 `Cargo.lock`，提交并推送 `main`，然后创建并推送对应 tag。例如当前版本为 `0.1.0`：

```powershell
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

不要复用已发布的 tag；后续发布先更新版本号。

发布前需要明确选择 Slint 的 GPLv3、Royalty-free attribution 或商业许可路径。
