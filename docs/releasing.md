# 发布 KeyCrash

Release Action 并行构建 Windows x64（含 x86 辅助组件）、Mac arm64 和 Mac x86_64。Windows 运行 workspace 测试并验证程序图标；Mac 运行主包测试、编译原生 fixture、验证 `.app` 架构与 ad-hoc 签名。全部成功后统一上传三个 ZIP 并生成 **draft Release**，不会自动公开。带预发布后缀的版本会标记为 prerelease。

v0.2.0 因 Mac 占用软件定位未完成已撤回。后续发布必须先完成正确进程归因、无关进程阴性对照、释放后不再命中和真实桌面交互验收，再在已有发布授权范围内将草稿公开。不得仅凭构建通过发布；旧标签保留源码历史，不代表有效发行版。

Mac 包尚未做 Developer ID 签名或 Apple 公证，Release 说明必须保留此限制及实机验收状态。普通 CI 构建不等于桌面快捷键和 UI 已验收。

## 发布新版本

1. 更新根目录 `Cargo.toml` 的版本号，并同步 `Cargo.lock`；提交并推送到 `main`。
2. 在要发布的提交上创建并推送匹配版本号的标签：

   ```powershell
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. 在 GitHub 的 Actions → Release 查看结果；成功后检查 draft Release 的完整压缩包，完成上述验收后再公开。

标签必须与该提交的 `Cargo.toml` 版本完全一致，例如 `0.1.0` 对应 `v0.1.0`。仅推送分支不会发布。

## 手动运行

在 Actions → Release → Run workflow 选择 `main`，填写已经推送的版本标签；流程会检出该标签的代码进行构建。也可以使用：

```powershell
gh workflow run release.yml --ref main -f tag=v0.1.0
```

手动入口使用 GitHub 的 [workflow_dispatch](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow?tool=cli)。标签不存在或版本不匹配时会失败。发布失败且尚未创建 Release 时，可以重新运行失败任务或使用手动入口；已有 Release 不会被覆盖，应发布新的版本。
