# Git 与版本发布流程

## 仓库与分支

仓库：https://github.com/Zukelo/handy_kit

- `main`：稳定基线与版本发布来源，GitHub 默认分支。
- `develop`：日常开发主分支，本机默认停留此分支。
- 可选 `feature/功能名`、`fix/问题名`：从 develop 创建，完成后合回 develop。
- 开发完成验证后，通过 PR 把 develop 合入 main，再打版本标签。

本轮不强制设置分支保护，也不改变仓库公开性。后续多人协作时再配置审批人数和保护规则。

## 本机开发

```sh
git switch develop
git pull --ff-only
cd development/apps/desktop
npm ci
npm run tauri:dev
```

Rust 使用仓库 `rust-toolchain.toml` 的 stable 工具链。若本机有 Homebrew Rust 与 rustup 共存，确保 rustup 的 shim 优先：

```sh
export PATH="$HOME/.cargo/bin:$PATH"
rustc --version
```

开发构建只针对本机平台：

```sh
npm run build:dev     # debug 可执行文件，不生成安装包
npm run package:dev   # debug 本机安装包
```

默认产物在 `development/apps/desktop/src-tauri/target/debug/`，本机安装包在该目录的 `bundle/`。开发包不上传 Releases。GUI 网页预览用 `npm run dev`。

## 持续检查

`.github/workflows/ci.yml` 在 main/develop 推送及面向这两条分支的 PR 时执行：

- npm ci 与前端构建；
- Rust 格式检查；
- 迁移及发布工具测试；
- 版本一致性检查。

日常提交不自动构建跨平台安装包。生产构建只在版本标签推送或显式手动触发 Release 工作流时运行。

## 发布一个版本

1. 将以下文件的版本设为同一值（示例 `0.1.0-rc.1`）：
   - `development/apps/desktop/package.json`
   - `development/apps/desktop/src-tauri/tauri.conf.json`
   - `development/apps/desktop/src-tauri/Cargo.toml`
2. 在 desktop 目录运行 `npm install --package-lock-only` 更新 npm 锁文件；运行 `cargo check` 更新应用包在 Cargo.lock 中的版本。
3. 更新 `CHANGELOG.md`，验证后提交 develop，再通过 PR 合入 main。
4. 在 main 上创建并推送对应标签：

```sh
git switch main
git pull --ff-only
python3 scripts/check_version.py --tag v0.1.0-rc.1
git tag -a v0.1.0-rc.1 -m 'HandyKit 0.1.0-rc.1'
git push origin v0.1.0-rc.1
```

Release 工作流先核验标签和三处版本一致，并确认标签提交在 main 历史上，然后并行构建：

|平台|架构|产物|
|---|---|---|
|Windows|x64|online / offline 两个 NSIS exe|
|macOS|Apple Silicon arm64|dmg|
|macOS|Intel x64|dmg|
|Linux|x64|deb、AppImage|

所有构建成功后，汇总六个安装包和 SHA256SUMS.txt 到同一 GitHub Release。包含 `-` 的版本标签自动标为预发布，其余标为正式版本。失败不发布新版本；可以在 Actions 中重跑失败任务。手动触发时填写已经存在的完整标签名，并选择 main 分支上的工作流。

版本文件不匹配、标签不在 main 历史或安装包数量异常时会失败，不会发布缺少平台包的版本。已发布版本不要移动标签或复用版本号，修复后递增版本。

## 当前签名和离线边界

- macOS 当前使用 ad-hoc 签名；未配置开发者证书和 Apple 公证，尚不具备正式商业分发签名。
- Windows 暂无代码签名证书。
- Windows offline 随包携带 WebView2 离线安装器；online 可联网下载。
- macOS 依赖系统 WKWebView。Linux 使用系统 WebKitGTK，AppImage 不等于全系统依赖离线内置；Linux 完整离线交付仍需按发行版验收。
- npm 与 Cargo 锁文件提交到 Git；CI 使用 npm ci 和 Cargo --locked。
- 应用自动更新尚未接入，本次提供的是 GitHub Releases 版本和安装包管理。

## 权限

检查和构建 job 只有 contents:read。仅汇总发布 job 使用 GitHub 自动提供的 GITHUB_TOKEN 的 contents:write，不需要新增个人访问令牌。仓库须允许 GitHub Actions 运行。

官方参考：https://v2.tauri.app/distribute/pipelines/github/
