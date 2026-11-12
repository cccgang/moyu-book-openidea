# Thief Book for OpenIDEA

摸鱼看小说插件：IDE 底部工具窗口显示当前页（默认 5 行、行距 1.5em = 字高一半的间隔），
状态栏显示书名与进度，全局快捷键翻页，老板键一键把面板变成无害的 TODO 清单。
阅读进度、全部设置持久化，重启 IDE 无损。支持 **UTF-8（含 BOM）/ UTF-16 / GB18030(GBK)** 编码的 txt（≤16MiB）。

要求 OpenIDEA **v0.1.42+**（插件 API 1.5）。

## 安装

- Settings › Plugins › **Install Plugin from Disk…**，选择本项目文件夹（含 `plugin.toml` 与 `dist/plugin.wasm`）。
- 或开发模式：把本目录放进项目的 `.openidea/plugins/thief-book-openidea/`，*Reload Plugins* 热加载。

## 放书（重要）

书库是**插件私有目录**（读它零权限）。装好后运行命令
**`Thief Book: 书库路径（往这里放 txt）`** —— 会弹出路径并复制到剪贴板，
把 txt 小说丢进去即可；再运行 **`Thief Book: 载入示例小说`** 可直接体验。

## 快捷键（可在 Keymap 改）

| 键 | 动作 |
|---|---|
| `Alt+Y` | 上一页 |
| `Alt+L` | 下一页 |
| `Alt+I` | **老板键**（隐藏/恢复） |

**面板内（焦点在面板时）同样全部可用**：工具条 `◂ ▸ 章 换 隐` 按钮、
`PgUp/PgDn/←/→/空格/回车`、`Alt+Y/L/I`。点击正文与滚轮**不**换页（0.2.3 起），
滚轮只在页内滚动（段落折行超出面板高度时）。
（v0.2.0 起面板为 web 视图：html body 的脚本会被 Tauri 全局 CSP 静默拦截——上游缺陷——所以迁移到 `openidea-plugin:` 协议加载的 web 页面。）

## 命令（Search Everywhere › Actions 搜 "Thief Book"）

翻页/老板键之外：上一章 / 下一章 / 跳页（状态栏项点击同效）/ 章节目录 / 换书 /
字号± / 每页行数± / 行距± / 每行字数± / 书库路径 / 载入示例小说。

默认排版：字号 14px · 每页 **5 个段落** · 行距 1.5em。**一行 = txt 的一个自然段**：
长段超出面板宽度自动折行（5 段可能显示成 6~8 视觉行，原版 thief-book 行为），超出面板高度时滚轮页内滚动。
老板键隐藏时，翻页/跳页/章节命令静默忽略，防盲翻错位。

## 识别章节

`第X章/卷/节/回/部/集/篇`（X 为数字或中文数字，整行 ≤24 字）或 `Chapter N`。

## 构建

```sh
rustup target add wasm32-wasip2
cargo test                                        # 主机单测（mock host）
cargo build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/thief_book_openidea.wasm dist/plugin.wasm
```

SDK 依赖锁在 `openidea` 仓库 tag `v0.1.42`（插件 API 1.5）。

## 手测清单

1. 载入示例小说 → 面板显示 5 行正文，行距为字高一半；状态栏出现 `📖 示例小说 0%`
2. `Alt+L` / `Alt+Y` 翻页，进度百分比随之变化且重启 IDE 后保持
3. 下一章/上一章跳转正确；章节目录可选中跳转
4. 放一本 GBK 编码 txt → 换书 → 正常显示无乱码
5. `Alt+I` → 面板变“本周待办”、状态栏小说项消失；再按恢复
6. 字号/行数/行距/每行字数命令生效（气泡反馈当前值），重启后保持
7. 老板模式下按 `Alt+L` → 无反应（静默忽略）
