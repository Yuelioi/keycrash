# 发布 KeyCrash

Release Action 会运行 workspace 测试、构建 Windows x64 主程序和 x86 辅助组件、验证程序图标，然后上传 `keycrash-v<版本号>-windows-x64.zip` 并生成 GitHub Release 说明。带预发布后缀的版本会标记为 prerelease。

## 发布新版本

1. 更新根目录 `Cargo.toml` 的版本号，并同步 `Cargo.lock`；提交并推送到 `main`。
2. 在要发布的提交上创建并推送匹配版本号的标签：

   ```powershell
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. 在 GitHub 的 Actions → Release 查看结果；成功后可在 Releases 下载完整压缩包。

标签必须与该提交的 `Cargo.toml` 版本完全一致，例如 `0.1.0` 对应 `v0.1.0`。仅推送分支不会发布。

## 手动运行

在 Actions → Release → Run workflow 选择 `main`，填写已经推送的版本标签；流程会检出该标签的代码进行构建。也可以使用：

```powershell
gh workflow run release.yml --ref main -f tag=v0.1.0
```

手动入口使用 GitHub 的 [workflow_dispatch](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow?tool=cli)。标签不存在或版本不匹配时会失败。发布失败且尚未创建 Release 时，可以重新运行失败任务或使用手动入口；已有 Release 不会被覆盖，应发布新的版本。
