# ALTRun v2 (ALTRunRT)

> 原版 ALTRun（Delphi 2007）的现代化重写版，基于 **Rust + Tauri + React + TypeScript** 构建，支持 Windows、macOS、Linux。

---

## 致谢

本项目基于 [etworker](https://github.com/etworker) 开发的原版 [ALTRun](https://github.com/etworker/ALTRun) 重写而来。

原版 ALTRun 是一款用 Delphi 2007 编写的 Windows 快速启动器，设计简洁、功能实用，深受用户喜爱。感谢 etworker 的出色工作和开源精神，为本项目提供了完整的功能参考和设计灵感。

---

## 项目背景

原版 ALTRun 是一款 12 年前用 Delphi 2007 编写的 Windows 快速启动器，核心思路是：按下热键弹出一个小窗口，输入关键字快速搜索并启动程序、打开文件夹、执行系统命令或搜索网页。

本项目完整重写了原版的全部核心功能，并在此基础上做了若干改进：

- 使用 **Rust** 替代 Delphi，性能更好、内存更安全
- 使用 **Tauri v2** 作为桌面框架，体积小、无 Electron 依赖
- 使用 **React + TypeScript** 构建前端 UI，支持深色/浅色主题切换
- 兼容原版 `ShortCutList.txt` 和 `ALTRun.ini` 文件格式（Windows）
- 新增导入/导出、开机自启、多参数占位符等功能
- **跨平台支持**：Windows、macOS、Linux

---

## 平台支持

| 功能 | Windows | macOS | Linux |
|------|:-------:|:-----:|:-----:|
| 全局热键 | ✅ | ✅ | ✅ |
| 系统托盘 | ✅ | ✅ | ✅ |
| 启动程序/URL | ✅ `ShellExecuteW` | ✅ `open` | ✅ `xdg-open` |
| 打开目录 | ✅ `explorer.exe` | ✅ `open` | ✅ `xdg-open` |
| 单实例保护 | ✅ 命名 Mutex | ✅ 文件锁 | ✅ 文件锁 |
| 开机自启 | ✅ 注册表 `HKCU\...\Run` | ✅ `~/Library/LaunchAgents/` plist | ✅ `~/.config/autostart/` desktop |
| 数据目录 | exe 同目录 | `~/Library/Application Support/ALTRun/` | `~/.config/ALTRun/` |
| 环境变量 | `%VAR%` | `$VAR` / `~` | `$VAR` / `~` |

---

## 功能特性

### 核心功能

| 功能 | 说明 |
|------|------|
| **全局热键唤出** | 按下热键（默认 `Ctrl+Space` 或 `Pause`）弹出/隐藏主窗口 |
| **关键字实时搜索** | 输入关键字即时过滤快捷项列表，支持正则表达式和通配符（`*` `?`） |
| **智能排序** | 根据使用频率 + 匹配位置加权排序，常用的排在前面 |
| **命令执行** | 静默启动程序，不弹出命令行窗口 |
| **参数传递** | 输入 `关键字 参数` 格式，空格后的内容作为参数传入 |
| **多参数支持** | 命令行中使用 `{%1}` `{%2}` ... `{%9}` 占位符，按位置填入参数 |
| **数字键快捷执行** | `Alt+数字` 或 `Ctrl+数字` 直接执行列表中对应位置的快捷项 |
| **系统托盘** | 左键单击切换显示/隐藏，右键弹出菜单（Show / Quit） |
| **使用频率记忆** | 每次执行自动 +1 并持久化保存 |
| **单实例保护** | 防止重复启动，第二个实例直接退出 |
| **开机自启动** | 无需管理员权限，各平台使用原生机制 |

### 参数占位符

| 占位符 | 说明 |
|--------|------|
| `{%1}` `{%2}` ... `{%9}` | 按位置填入第 N 个参数（空格分隔） |
| `{%p}` 或 `%p` | 所有参数合并为一个字符串 |
| `{%c}` | 剪贴板内容（或所有参数） |

**示例：**
- 命令行 `notepad`，输入 `notepad test.txt` → 执行 `notepad test.txt`
- 命令行 `copy {%1} {%2}`，输入 `cp a.txt b.txt` → 执行 `copy a.txt b.txt`
- 命令行 `https://www.google.com/search?q=`（UTF8Query 类型），输入 `g hello` → 打开 Google 搜索 hello

### 参数类型

| 类型 | 说明 |
|------|------|
| `No Param` | 无参数，直接执行命令行 |
| `Param (no encoding)` | 参数原样传入，不做编码 |
| `URL Query encoding` | 参数做 URL 编码（适合百度等搜索） |
| `UTF-8 Query encoding` | 参数做 UTF-8 编码（适合 Google 等搜索） |

### 显示控制前缀（Windows）

在命令行开头加前缀可控制窗口显示方式：

| 前缀 | 效果 |
|------|------|
| `@+` | 最大化启动 |
| `@-` | 最小化启动 |
| `@` | 隐藏窗口启动 |
| （无前缀） | 正常显示 |

### 配置系统

**Alt+C** 打开配置面板，可调整：

| 分类 | 配置项 |
|------|--------|
| **主题** | 深色 / 浅色主题切换（实时预览） |
| **导入/导出** | 一键导出所有配置和快捷项为 JSON，跨设备迁移 |
| **热键** | 主热键 / 副热键（支持 Ctrl、Alt、Shift、Win/Cmd 组合） |
| **行为** | 开机自启、正则搜索、任意位置匹配、数字键快捷执行等 |
| **外观** | 窗口宽度、透明度、圆角半径、自动隐藏延迟 |

### 开机自启动

在配置面板中勾选 **"Launch at Windows startup"** 并保存，各平台实现方式：

- **Windows**：写入注册表 `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`
- **macOS**：写入 `~/Library/LaunchAgents/com.altrun.app.plist`
- **Linux**：写入 `~/.config/autostart/altrun.desktop`

均无需管理员权限，取消勾选并保存即可移除。

### 键盘快捷键

| 按键 | 功能 |
|------|------|
| `↑` / `↓` 或 `Tab` / `Shift+Tab` | 在结果列表中移动选择 |
| `Enter` | 执行选中项（无匹配时弹出添加对话框） |
| `Esc` | 清空输入框（再按一次隐藏窗口） |
| `Alt+数字` / `Ctrl+数字` | 直接执行列表中第 N 项 |
| `Ctrl+D` | 打开选中快捷项的所在目录 |
| `Ctrl+C` | 复制选中快捷项的命令行到剪贴板 |
| `Alt+S` | 打开快捷项管理器 |
| `Alt+C` | 打开配置面板 |

---

## 数据文件

| 平台 | 数据目录 |
|------|---------|
| Windows | exe 所在目录（兼容原版 ALTRun） |
| macOS | `~/Library/Application Support/ALTRun/` |
| Linux | `~/.config/ALTRun/` |

各平台数据文件相同：

| 文件 | 说明 |
|------|------|
| `ALTRun.ini` | 配置文件（INI 格式） |
| `ShortCutList.txt` | 快捷项列表（兼容原版格式） |
| `altrun_debug.log` | 调试日志 |

### ShortCutList.txt 格式

```
F<频率>    |<参数类型>          |<关键字>              |<名称>                        |<命令行>
F100      |                    |Computer              |My Computer                   |::{20D04FE0-3AEA-1069-A2D8-08002B30309D}
F50       |URL_Query           |b                     |Baidu Search                  |https://www.baidu.com/s?wd=
F50       |UTF8_Query          |g                     |Google Search                 |https://www.google.com/search?q=
```

参数类型取值：空（无参数）、`No_Encoding`、`URL_Query`、`UTF8_Query`

---

## 技术架构

```
altrun/
├── src/                        # 前端 React + TypeScript
│   ├── App.tsx                 # 主界面：搜索框、结果列表、状态栏
│   ├── ShortCutManager.tsx     # 快捷项管理器（CRUD）
│   ├── ConfigPanel.tsx         # 配置面板（含主题切换、导入导出、开机自启）
│   ├── types.ts                # 共享类型定义
│   └── styles.css              # 全局样式（深色/浅色主题变量）
│
└── src-tauri/                  # Rust 后端
    └── src/
        ├── main.rs             # 程序入口
        ├── lib.rs              # Tauri 命令、热键、托盘、单实例、开机自启（跨平台）
        ├── models.rs           # 数据结构：ShortCutItem、AppConfig、ExportData
        ├── storage.rs          # 文件读写、数据目录（跨平台）
        ├── search.rs           # 搜索过滤与排序算法
        └── executor.rs         # 命令执行引擎（跨平台：ShellExecuteW / open / xdg-open）
```

### 前端技术栈

- **React 18** + **TypeScript** — UI 框架
- **Vite 6** — 构建工具
- **@tauri-apps/api** — 与 Rust 后端通信
- **@tauri-apps/plugin-dialog** — 文件选择对话框

### 后端技术栈

- **Rust 1.87+**
- **Tauri v2** — 桌面框架（窗口管理、系统托盘、IPC）
- **tauri-plugin-global-shortcut** — 全局热键注册
- **tauri-plugin-dialog** — 文件选择对话框（导入导出）
- **tauri-plugin-shell** — Shell 命令支持
- **regex** — 正则表达式搜索
- **urlencoding** — URL/UTF-8 参数编码
- **serde / serde_json** — JSON 序列化（导入导出）
- **libc** — Unix 文件锁（单实例保护，仅 macOS/Linux）
- **dirs** — 跨平台目录路径

### 核心设计

**搜索排序算法**（沿用原版 ALTRun 加权公式）：

```
rank = 1024 + freq × 4 - match_pos × 128 - len_diff × 16
```

**命令执行**（跨平台）：
- Windows：`ShellExecuteW` — 不弹出命令行窗口，支持程序、URL、系统命令、Shell GUID
- macOS：`open` — 使用系统默认程序打开文件/URL/App
- Linux：`xdg-open` — 使用桌面环境默认程序

**单实例保护**（跨平台）：
- Windows：命名 Mutex `ALTRun_SingleInstance_Mutex_v2`
- macOS/Linux：`/tmp/altrun.lock` 文件排他锁（`flock`）

**开机自启**（跨平台，均无需管理员权限）：
- Windows：注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- macOS：`~/Library/LaunchAgents/com.altrun.app.plist`（launchd）
- Linux：`~/.config/autostart/altrun.desktop`（XDG Autostart）

---

## 与原版 ALTRun 的对比

| 特性 | 原版（Delphi） | 本版（Rust+Tauri） |
|------|--------------|-----------------|
| 运行时依赖 | VCL 运行库 | 无（静态编译） |
| 平台支持 | Windows only | Windows / macOS / Linux |
| 文件格式兼容 | — | ✅ 完全兼容（Windows） |
| 全局热键 | ✅ | ✅ |
| 正则/通配符搜索 | ✅ | ✅ |
| 参数传递 | ✅ | ✅ 增强（多参数占位符） |
| 使用频率排序 | ✅ | ✅ |
| 数字键快捷执行 | ✅ | ✅ |
| 系统托盘 | ✅ | ✅ |
| 开机自启 | ✅ | ✅ |
| 单实例保护 | ✅ | ✅ |
| 深色/浅色主题 | ❌ | ✅ |
| 导入/导出 | ❌ | ✅ |
| 窗口大小可调 | ❌ | ✅ |
| 命令行窗口弹出 | ❌（有） | ✅（无） |

---

## 构建方法

### 环境要求

- Rust 1.87+（`rustup` 安装）
- Node.js 18+
- Windows 10/11、macOS 11+、或主流 Linux 发行版

### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/cngzblank/ALTRunRT.git
cd ALTRunRT

# 安装前端依赖
npm install

# 构建发布版本
npx tauri build
```

构建产物：
- **Windows**：`src-tauri/target/release/altrun.exe`
- **macOS**：`src-tauri/target/release/bundle/macos/ALTRun.app`
- **Linux**：`src-tauri/target/release/bundle/appimage/altrun.AppImage`

---

## 默认快捷项

首次运行时根据平台自动生成默认快捷项：

### Windows
| 关键字 | 名称 | 命令 |
|--------|------|------|
| `Computer` | My Computer | 打开"此电脑" |
| `notepad` | Notepad | 记事本 |
| `cmd` | Command Prompt | 命令提示符 |
| `calc` | Calculator | 计算器 |
| `g` | Google Search | Google 搜索 |
| `b` | Baidu Search | 百度搜索 |

### macOS
| 关键字 | 名称 | 命令 |
|--------|------|------|
| `finder` | Finder | Finder 文件管理器 |
| `terminal` | Terminal | 终端 |
| `safari` | Safari | Safari 浏览器 |
| `calculator` | Calculator | 计算器 |
| `prefs` | System Preferences | 系统偏好设置 |
| `g` | Google Search | Google 搜索 |

---

## License

MIT

---

## 相关链接

- 原版 ALTRun：https://github.com/etworker/ALTRun
- 本项目仓库：https://github.com/cngzblank/ALTRunRT
