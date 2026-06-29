# etlp 油猴脚本

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Linux](https://img.shields.io/badge/Linux-FCC624?logo=linux&logoColor=black)](https://github.com/PiliPili-Team/etlp/releases)
[![macOS](https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white)](https://github.com/PiliPili-Team/etlp/releases)
[![Windows](https://img.shields.io/badge/Windows-0078D4?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9Ii0yIC0yIDI4IDI4IiBmaWxsPSJ3aGl0ZSI%2BPHBhdGggZD0iTTAgMy40NDkgOS43NSAyLjF2OS40NTFIMG0xMC45NDktOS42MDJMMjQgMHYxMS40SDEwLjk0OU0wIDEyLjZoOS43NXY5LjQ1MUwwIDIwLjY5OU0xMC45NDkgMTIuNkgyNFYyNGwtMTIuOS0xLjgwMSIvPjwvc3ZnPg==&logoColor=white)](https://github.com/PiliPili-Team/etlp/releases)

GitHub：<https://github.com/PiliPili-Team/etlp>

**etlp** 是一个原神驱动的媒体播放器桥接工具，主要面向 Emby。
它在本机运行轻量 HTTP 服务，接收浏览器油猴脚本发送的播放请求，
再交给本地播放器处理，同时支持播放列表构建、播放进度回写和可选的
第三方观看记录同步。

## 首要特性

- 支持 **mpv · iina · vlc · mpc-hc · potplayer · dandanplay**
- 播放列表管理，支持版本和字幕偏好筛选
- Emby 播放进度回写（Jellyfin：**实验性，未充分测试**）
- Trakt.tv 与 Bangumi.tv 观看记录同步
- 并发下载管理，支持暂停、恢复和限速
- macOS 与 Windows 原生 GUI（Tauri，macOS 支持毛玻璃效果）

## 油猴脚本特性

- 默认优先使用 `/etlp`，必要时自动回退到旧版 `/embyToLocalPlayer`
- Plex 保持 `/plexToLocalPlayer` 兼容
- 可在油猴菜单中配置本地服务端口
- 使用网页内本地化通知，不再弹出浏览器 alert
- 通过更早拦截播放链路，避免 Emby 不兼容流提示闪屏

> [!IMPORTANT]
> 原神主题图标版权归属于米哈游（miHoYo）公司所有，使用皆因为热爱。
> 如果该使用方式构成侵权，将立即删除。

## 安装配套 APP

1. 打开 Release 页面：
   <https://github.com/PiliPili-Team/etlp/releases>
2. 下载对应平台的安装包。
3. 安装并启动应用。
4. 确保油猴脚本端口和 APP 服务端口一致。

推荐下载：

- macOS：下载 `.dmg`，将应用拖入 `Applications`。
- Windows：下载 `.msi` 或 `.exe`，运行安装器。
- Windows Portable：下载 `.zip`，解压后运行 `Genshin.exe`。

默认服务端口是 `58000`。

## 首次启动安全提示

当前发布包没有使用付费 Apple / Microsoft 开发者证书签名，所以 macOS
或 Windows 可能会在首次启动时显示系统安全提示。这是未签名构建的
常见现象。

### macOS：提示应用已损坏或无法打开

如果 macOS 提示 `Genshin` 已损坏或无法打开，执行一次下面的命令移除
下载文件的隔离属性，然后重新打开应用：

```bash
sudo xattr -dr com.apple.quarantine /Applications/Genshin.app
```

### Windows：Defender、防火墙或杀毒软件提示

允许应用通过 Windows 防火墙，并按需加入杀毒软件白名单。遇到 SmartScreen
提示时，选择 **更多信息**，再选择 **仍要运行**。

### Windows Portable：放在 `C:\` 或 `Program Files` 后打不开

便携版会把 `config/` 和 `data/` 写在可执行文件旁边。磁盘根目录、
`C:\Program Files\` 或带有严格 NTFS 权限的目录可能需要管理员权限。
建议放到当前用户可写的位置，例如：

```text
C:\Users\<you>\Apps\etlp\
```

如果应用请求 UAC 提权，可以允许提权；也可以把便携版目录移动到
当前用户有写入权限的位置。

## 更多说明

完整排障内容请阅读主 README：

<https://github.com/PiliPili-Team/etlp/blob/main/README.md>
