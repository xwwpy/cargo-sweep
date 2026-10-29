# Cargo Sweep

Cargo Sweep 是一个 Rust 项目构建缓存清理工具：扫描并删除 `target` 目录释放磁盘空间，并通过统一的 Rust 环境页面管理工具链、Cargo 下载源和存储设置。没有 Rust 环境时，可以在应用内一键安装 rustup 和 Stable 工具链。

## 支持平台

| 平台 | 架构 | 安装包 |
| --- | --- | --- |
| Windows 10 / 11 | x64 | NSIS 安装包、Portable 便携版 |
| macOS 11+ | Intel / Apple Silicon | Universal DMG |
| Linux | x86_64 | DEB、AppImage |

安装包统一从 [GitHub Releases](https://github.com/xwwpy/cargo-sweep/releases/latest) 下载。以下命令以 **0.7.10** 为例，后续版本替换版本号即可。

## 安装

### Windows

下载 `Cargo-Sweep-0.7.10-installer-x64.zip`，解压后运行 `INSTALL-CARGO-SWEEP.cmd`。也可以使用 `Cargo-Sweep-0.7.10-portable-x64.zip` 免安装运行。

### macOS

下载 `Cargo-Sweep-0.7.10-universal.dmg`，打开后将 Cargo Sweep 拖入「应用程序」。

应用未签名，首次启动请右键 Cargo Sweep 选择「打开」。如仍被拦截，可执行：

```bash
xattr -dr com.apple.quarantine "/Applications/Cargo Sweep.app"
```

如需始终允许读取任意目录，点击应用侧栏的「始终允许读取」，在系统设置的「完全磁盘访问权限」中添加 Cargo Sweep。

### Linux

Debian / Ubuntu：

```bash
sudo apt install ./Cargo-Sweep-0.7.10-amd64.deb
```

如果 `apt` 无法直接安装本地包：

```bash
sudo dpkg -i ./Cargo-Sweep-0.7.10-amd64.deb
sudo apt-get install -f
```

其他发行版可使用 AppImage：

```bash
chmod +x Cargo-Sweep-0.7.10-amd64.AppImage
./Cargo-Sweep-0.7.10-amd64.AppImage
```

> Linux 建议使用 Ubuntu 24.04 或更新版本；DEB 依赖 WebKitGTK 4.1。
