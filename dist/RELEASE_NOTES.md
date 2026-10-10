# XMind-RS v0.1.0

首个发布版本。用 Rust 从零实现的开源思维导图：**完全本地运行，无账号、无订阅、无遥测**，与 `.xmind` 文件格式互通（官方 XMind 可打开本软件保存的文件，本软件可打开官方 Zen/2020+ 及 XMind 8 老格式文件）。

## 下载

| 文件 | 说明 |
|---|---|
| `XMind-RS-v0.1.0-macOS-arm64.zip` | macOS 图形界面应用（Apple Silicon），解压得到 `XMind-RS.app` |
| `xmind-cli-v0.1.0-macOS-arm64.tar.gz` | 命令行工具 `xmind-cli` + 4 个示例 .xmind 文件 |

Windows / Linux / Intel Mac：`cargo build --release -p xmind-app` 从源码构建（依赖仅 Rust 工具链）。

> 未签名应用首次运行如被 macOS 拦截：`xattr -cr /Applications/XMind-RS.app`

## 主要功能

- **9 种导图结构**：思维导图 / 逻辑图（左）/ 双向导图 / 组织结构图（上、下）/ 树形图 / 时间线 / 鱼骨图 / 矩阵图，同图可混用
- **编辑**：Tab 建子主题、Enter 建同级、双击就地编辑、拖拽主题到其他主题上移动层级、框选多主题、撤销/重做（120 步）、复制粘贴整个子树
- **内容元素**：优先级 / 任务进度饼图 / 旗 / 星 / 人物 / 表情等矢量标记、标签、备注、超链接、联系线、外框、概要、图片
- **主题样式**：9 套内置主题；节点级填充色 / 文字色 / 边框 / 6 种形状 / 字号 / 加粗
- **视图**：无限画布（触控板平移缩放）、大纲面板、属性面板、查找替换、下钻、演示模式、多画布
- **文件**：自动保存 + 崩溃恢复、最近文件、拖入文件直接打开、中英双语、8 个内置模板
- **导入导出**：导入 Markdown / OPML / FreeMind / 缩进文本；导出 PNG / SVG / PDF / Markdown / 纯文本 / OPML / FreeMind / HTML / Word / Excel
- **CLI**：`xmind-cli new|convert|render|info` 无头建图、格式转换、渲染出图，可集成脚本

## 快捷键速查

| 按键 | 功能 |
|---|---|
| Tab / Enter | 子主题 / 同级主题 |
| F2 / 双击 | 编辑 |
| ⌘Z / ⌘⇧Z | 撤销 / 重做 |
| ⌘F / ⌘P | 查找 / 演示模式 |
| ⌘S / ⌘O / ⌘N | 保存 / 打开 / 新建 |
| Esc | 取消编辑 / 退出下钻 / 清除选择 |

## 校验

`SHA256SUMS` 附带全部产物的 SHA-256 校验值。

## 已知限制

- PDF 导出中的文字为高分辨率位图（未嵌入字体子集，CJK 场景下最稳妥）
- 未实现：甘特图、官方贴纸库、音频备注、Word/Excel 导入；云同步/AI/协作为订阅功能，明确不做

---

MIT License · Rust + egui · 与 XMind Ltd. 无关联
