# Moyu Book for OpenIDEA 📖（摸鱼看书）

> 一个 [OpenIDEA](https://github.com/l0ng-ai/openidea) 摸鱼插件：在 IDE 底部的工具窗口里读 txt 小说，全局快捷键翻页，老板键一键隐身。
> A stealth txt-novel reader for OpenIDEA — read in a bottom tool window, flip pages with hotkeys, vanish with a boss key.

![插件 API](https://img.shields.io/badge/插件_API-1.5_(OpenIDEA_≥_v0.1.42)-orange) ![License](https://img.shields.io/badge/License-MIT-green)

灵感来自 IDEA 平台的同类插件 [thief-book-idea](https://github.com/yisier/thief-book-idea)。本项目用 OpenIDEA **官方插件 API**（WASM 组件）重新实现：不改 IDE 本体、无任何私有补丁，装上即用。

## 效果预览

```
┌─ OpenIDEA ────────────────────────────────────┐
│  main.rs  │  ← 代码区一切如常                  │
├───────────┴───────────────────────────────────┤
│ ◂ ▸ 章 换 隐                                  │ ← 工具条（单字符，低调）
│ 李慕白握紧长剑，沉声道："请。"                 │
│ 那人冷笑一声，剑尖微颤，一点寒芒直取他咽喉。   │ ← 阅读面板（底部工具窗）
│ ……                                            │    默认 5 个自然段 / 页
├───────────────────────────────────────────────┤
│ ✓ Rust main* Ln 42  │  📖 剑气长江 · 37%      │ ← 状态栏进度（点击跳页）
└───────────────────────────────────────────────┘
```

## 特性

- 📖 **底部工具窗口**阅读面板，与代码互不打扰；状态栏常驻 📖 书名 + 进度
- ⌨️ **全局快捷键**翻页：`Alt+Y` 上一页 / `Alt+L` 下一页 / `Alt+I` 老板键（可在 Keymap 改绑）
- 🕶️ **老板键**：面板瞬间换成无害的"本周待办"清单，状态栏小说项同步消失，翻页命令静默忽略防盲翻；再按一键恢复
- 📃 **按自然段分页**：一"行" = txt 里的一个段落，长段在面板内自动折行；字号 / 每页行数 / 行距随时可调
- 🔖 **进度持久化**：阅读位置与全部设置写进插件存储，重启 IDE、插件闲置被回收都不丢
- 📚 **多书管理**：章节目录、上/下一章、跳页、换书
- 🈶 **编码自动识别**：UTF-8（含 BOM）/ UTF-16LE·BE / GB18030(GBK)，单文件 ≤ 16 MiB
- 🤫 **界面低调**：工具条只有 `◂ ▸ 章 换 隐` 五个单字符按钮

## 安装

### 方式一 · 下载安装包（免编译）

到 [Releases](https://github.com/cccgang/moyu-book-openidea/releases) 下载 `owncode.moyu-book-<版本>.oiplugin`，
OpenIDEA › Settings › Plugins › **Install Plugin from Package File…** 选中它即可。
OpenIDEA 支持单文件安装包（`.oiplugin` / `.zip`），不必选整个文件夹。

> 升级版本时先卸载旧插件并重启 IDE，再装新包，避免装到旧拷贝。

### 方式二 · 从源码构建

```sh
git clone https://github.com/cccgang/moyu-book-openidea.git
cd moyu-book-openidea
rustup target add wasm32-wasip2
cargo test                                        # 主机单测（可选）
cargo build --release --target wasm32-wasip2
mkdir dist && cp target/wasm32-wasip2/release/moyu_book_openidea.wasm dist/plugin.wasm
```

然后 OpenIDEA › Settings › Plugins › **Install Plugin from Disk…**，选择项目文件夹（含 `plugin.toml`）。
构建完成后重装/升级插件时，记得先卸载旧版并重启 IDE 再装，避免装到旧拷贝。

打自己的单文件安装包（可选）：把 `plugin.toml`、`dist/plugin.wasm`、`web/` 打成 zip——
包内容放在压缩包根目录或唯一顶层文件夹都可以——扩展名用 `.oiplugin`。
官方 CLI 的 `openidea-plugin pack` / `check` 能完成同样的打包与校验。

### 方式三 · 开发模式

把项目文件夹放进项目的 `.openidea/plugins/thief-book-openidea/`，执行 *Reload Plugins* 热加载。

> ⚠️ 需要 OpenIDEA **v0.1.42+**（插件 API 1.5）。更早版本没有 web 视图与 `read-bytes`，面板交互和 GBK 解码都会失效。

## 快速上手

1. 装好后运行命令 **`Moyu Book: 载入示例小说`** 直接体验；或运行 **`Moyu Book: 书库路径`** 拿到书库地址（自动复制到剪贴板），把你的 txt 小说丢进去。
2. `Alt+L` / `Alt+Y` 翻页；状态栏 📖 显示书名与进度，点击可跳页。
3. 老板来了按 `Alt+I`，走了再按一次。

书库是插件的私有目录，读取零权限，往里丢 `*.txt` 即完成加书。

## 快捷键

| 键 | 动作 |
|---|---|
| `Alt+Y` | 上一页 |
| `Alt+L` | 下一页 |
| `Alt+I` | **老板键**（隐藏 / 恢复） |

## 面板内操作

- **工具条**：`◂` 上一页 · `▸` 下一页 · `章` 章节目录 · `换` 换书 · `隐` 老板键（隐藏时变 `显`）
- **键盘**（面板获得焦点时）：`PgDn` / `→` / `空格` / `回车` 下一页，`PgUp` / `←` 上一页，`Alt+Y/L/I` 同全局
- 点击正文与滚轮**不会**翻页：滚轮仅在内容超出面板高度时做页内滚动（v0.2.3 起，防误触）

## 全部命令

Search Everywhere › Actions 搜 "Moyu Book" 即可看到：

| 分类 | 命令 |
|---|---|
| 阅读 | 上一页 / 下一页 / 跳页（状态栏点击同效） |
| 章节 | 下一章 / 上一章 / 章节目录 |
| 书库 | 换书 / 书库路径 / 载入示例小说 |
| 排版 | 字号 ± / 每页行数 ± / 行距 ± |
| 伪装 | 老板键（隐藏 / 恢复） |

## 章节识别

`第X章 / 卷 / 节 / 回 / 部 / 集 / 篇`（X 为阿拉伯数字或中文数字，整行 ≤ 24 字）或 `Chapter N`。

## 给 OpenIDEA 插件开发者

本项目踩过最深的坑，值得一提：`[[contributes.views]]` 的 `html` body 是 srcdoc iframe，会继承 Tauri 全局 CSP（`script-src 'self' 'wasm-unsafe-eval'`），**帧内所有脚本（含官方消息桥）被静默拦截**，表现为正文正常显示但交互全灭。需要交互 UI 的插件请用 `web = "web/index.html"` web 视图（`openidea-plugin:` 协议加载、独立 CSP、脚本可用）。

代码结构：命令分发 / 存储 / 状态栏在 [`src/lib.rs`](src/lib.rs)；解码、分页、章节识别等纯逻辑在 [`src/novel.rs`](src/novel.rs)（主机端 13 个单测，含 MockHost 推送断言）；面板页在 [`web/index.html`](web/index.html)。

## License

[MIT](LICENSE)
