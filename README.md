# iKunTools

> 离线优先的开发者工具箱 —— 一个不用登录、不联网也能干活的桌面小工具集。

![license](https://img.shields.io/badge/license-MIT-blue.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2021-000000?logo=rust&logoColor=white)
![tests](https://img.shields.io/badge/tests-138%20passed-brightgreen)
![platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)
[![Build Windows](https://github.com/Axuan4j/iKunTools/actions/workflows/build-windows.yml/badge.svg)](https://github.com/Axuan4j/iKunTools/actions/workflows/build-windows.yml)

JSON 格式化、正则调试、AES/RSA、JWT 解码、时间戳换算、HTTP 调试……这些事每天都在做，却总要在十几个网页之间来回切。iKunTools 把它们收进一个 18 MB 的原生窗口里：**打开就能用，断网也能用，输入的内容不会离开你的机器。**

## 特性

- **完全离线** —— 除「网络工具」里你主动发起的探测/请求外，没有任何网络访问。
- **无账号、无遥测、无云同步** —— 不收集数据，不上报用量，不需要登录。
- **重活交给 Rust** —— 加解密、正则、XML/JSON 互转、HTTP、离线 IP 库都在原生侧执行，界面不卡。
- **核心与界面解耦** —— 所有能力都是 `ikt-core` 里的纯函数，桌面端只是其中一层壳，将来可直接给 CLI / CI 复用。
- **单一 IPC 契约** —— 前后端字段统一 camelCase，每个模块都有契约测试锁定字段名。
- **键盘优先** —— <kbd>⌘K</kbd> 全局搜索任意工具，<kbd>⌘↵</kbd> 发送 HTTP 请求。

## 功能

### 数据格式

| 工具 | 能力 |
| --- | --- |
| JSON 格式化 | 校验、美化、压缩，报错精确定位到行/列并给出上下文片段；统计节点数、深度、对象与数组数量 |
| XML / JSON 互转 | 双向转换，属性、文本节点、重复节点自动映射，可选属性前缀与文本键名；另含 XML 格式化与压缩 |

### 文本处理

| 工具 | 能力 |
| --- | --- |
| 正则测试 / 替换 | 列出每处匹配的行列位置、字节区间与全部捕获组（含命名组），支持 `i` `m` `s` `x` `u` 标志与 `$1` / `$name` 替换；上限 2000 条防卡死 |

### 编码转换

| 工具 | 能力 |
| --- | --- |
| Base64 编解码 | 标准与 URL 安全字符集，可选去掉 padding，解码自动容错换行与缺失填充 |
| URL 编解码 | 百分号编码/解码，区分「组件」与「完整 URL」两种保留集，可选 `+` 表示空格与十六进制大小写 |
| UTF-8 / ASCII 互转 | 中文转 `\uXXXX`、`&#xXXXX;`、`U+XXXX` 三种风格；反向解码宽容识别并定位非法字符位置 |

### 加密解密

| 工具 | 能力 |
| --- | --- |
| 哈希 / HMAC | MD5、SHA-1、SHA-256、SHA-512、BLAKE3，以及 HMAC-SHA256，同时输出 hex 与 base64 |
| AES 加解密 | GCM / CBC / CTR / ECB，密钥支持口令派生（PBKDF2）或原始 key，可选 PKCS#7 填充、随机或自定义 IV |
| RSA 加解密 | 本地生成 2048 / 3072 / 4096 位密钥对并导出 PKCS#1 / PKCS#8 PEM，支持 OAEP、PKCS#1 v1.5 与混合封装；显示 SHA-256 与 OpenSSH 指纹 |
| JWT 解码 | 拆出 header / payload / signature，解析 `iat` `exp` `nbf` 与剩余有效期（仅解码，不验签） |

### 时间与随机

| 工具 | 能力 |
| --- | --- |
| 时间戳转换 | Unix 秒/毫秒/微秒/纳秒自动识别并与日期互转，展示 UTC、指定时区、星期、年内第几天、ISO 周次与相对时间 |
| 随机生成 | UUID v4 / v7、ULID、NanoID、随机密码、随机文本、随机字节、随机整数、清单抽取；估算熵值 |

### 网络工具

| 工具 | 能力 |
| --- | --- |
| HTTP 请求 | 类 Postman 的接口调试：方法、查询参数、请求头、Raw / Form / URL Encoded 请求体、Bearer 与 Basic 认证、超时/重定向/TLS 校验/代理；响应支持 JSON 美化、图片预览、二进制 Base64 与**一键复制为 cURL**（Bash / 单行 / PowerShell） |
| 网络探测 | TCP 握手测延迟（无需 root，不受 ICMP 屏蔽影响）、telnet 式收发并附十六进制视图、DNS 解析、IP 归属地查询 |

IP 归属地使用 [ip2region](https://github.com/lionsoul2014/ip2region) 的离线 xdb 库，11 MB 数据直接编译进二进制，不读磁盘路径、不联网查询。

## 安装

暂无签名后的正式发布版本，安装包从 CI 或本地构建获取：

- **Windows**：打开 [Build Windows](https://github.com/Axuan4j/iKunTools/actions/workflows/build-windows.yml) → 选最新一次成功的运行 → 下载 `iKunTools-windows-x64`，解压后是 NSIS 安装包（`-setup.exe`）与 `.msi`。
- **macOS**：本地执行 `pnpm tauri build --bundles dmg`，产物 `.dmg` 拖进「应用程序」即可。
- **绿色版**：直接运行可执行文件，但系统需已安装 WebView2 运行时（Win11 自带）。

> 当前构建未做代码签名，首次打开需右键「打开」，或执行 `xattr -dr com.apple.quarantine /Applications/iKunTools.app`。

## 从源码构建

### 环境要求

| 依赖 | 版本 |
| --- | --- |
| Node.js | ≥ 20 |
| pnpm | ≥ 9 |
| Rust | stable（macOS 用 `aarch64-apple-darwin` / Windows 用 `x86_64-pc-windows-msvc`） |
| 系统工具链 | macOS：Xcode Command Line Tools；Windows：VS Build Tools（C++ 桌面开发）+ WebView2 |

### 命令

```bash
pnpm install

# 开发模式（热更新，浏览器预览也能跑大部分工具）
pnpm dev            # 只起前端
pnpm tauri dev      # 带原生壳

# 打包
pnpm tauri build --bundles app        # macOS → .app
pnpm tauri build --bundles dmg        # macOS → .dmg
pnpm tauri build                      # Windows → msi + nsis（需在 Windows 上构建）
pnpm tauri build --no-bundle          # 只要可执行文件，绿色版

# 质量门禁
pnpm typecheck      # tsc --noEmit
pnpm test:core      # cargo test -p ikt-core
```

产物位于 `target/release/bundle/`（workspace 根目录，不是 `src-tauri/` 下）。

> Tauri 无法从 macOS 交叉编译出 Windows 安装包，Windows 产物必须在 Windows 环境（或 GitHub Actions 的 `windows-latest`）构建。

## 技术栈

| 层 | 选型 |
| --- | --- |
| 桌面壳 | Tauri 2 |
| 逻辑层 | Rust 2021 + serde / aes-gcm / rsa / regex / quick-xml / ureq / chrono / xdb-parse |
| 界面 | React 19 + TypeScript 5.7 |
| 构建 | Vite 6 |
| 样式 | Tailwind CSS 4（自定义语义色板，无组件库） |
| 编辑器 | CodeMirror 6（JSON 语法高亮、只读/可编辑切换） |

## 项目结构

```
iKunTools/
├── crates/ikt-core/          # 纯逻辑层：禁止出现 Tauri / UI 依赖
│   └── src/
│       ├── json.rs           # JSON 格式化 / 压缩 / 统计
│       ├── xml.rs            # JSON ↔ XML、XML 格式化 / 压缩
│       ├── encoding.rs       # Base64 / URL / ASCII
│       ├── regexp.rs         # 正则测试与替换
│       ├── crypto.rs         # 摘要、HMAC、AES
│       ├── rsa.rs            # RSA 密钥与加解密
│       ├── jwt.rs            # JWT 解码
│       ├── timeutil.rs       # 时间戳 ↔ 日期
│       ├── random.rs         # UUID / ULID / NanoID / 密码 / 随机数
│       ├── http.rs           # HTTP 请求组装与发送
│       └── net.rs            # TCP ping / telnet / DNS / IP 归属地
├── src-tauri/                # Tauri 壳：只做参数转发与线程调度
│   ├── src/commands.rs       # #[tauri::command(rename_all = "camelCase")]
│   ├── src/lib.rs            # 命令注册
│   ├── ip2region_v4.xdb      # 11 MB 离线归属地库（编译期 include_bytes!）
│   └── tauri.conf.json
└── src/                      # React 前端
    ├── features/             # 每个工具一个页面 + registry.ts 注册表
    ├── components/           # CodeMirror 封装、命令面板、通用控件与图标
    ├── lib/                  # api.ts（IPC 契约）/ curl.ts / devFallback.ts
    └── shell/Sidebar.tsx     # 侧栏与分组
```

## 架构说明

**三层分工**

1. `crates/ikt-core` 只放「输入 → 输出」的纯函数，可以脱离 Tauri 单独测试与复用，`cargo test -p ikt-core` 就能覆盖全部业务逻辑。
2. `src-tauri` 是薄壳：校验参数、`spawn_blocking` 调度、把错误转成字符串。这里不允许写业务逻辑。
3. `src` 只负责交互与展示，通过 `src/lib/api.ts` 这唯一入口调用原生能力。

**前后端契约**

所有结构体都带 `#[serde(rename_all = "camelCase")]`，每个模块配一个 `serializes_camel_case_contract` 测试，断言 TS 侧依赖的字段名真实存在。改字段会先让 Rust 测试挂掉，而不是运行时静默变 `undefined`。

**浏览器兜底**

`pnpm dev` 直接开网页时没有 Tauri 环境，`devFallback.ts` 用 WebCrypto 复刻了编码、哈希、AES、RSA、时间与随机等能力，方便快速改界面；socket 类工具（网络探测、HTTP）在浏览器里会明确提示「请在桌面端运行」，不会假装成功。

## 开发一个工具

1. **写逻辑**：在 `crates/ikt-core/src/` 新建模块，纯函数 + 单元测试（含契约测试），`lib.rs` 里 `pub mod` 导出。
2. **接命令**：`src-tauri/src/commands.rs` 加 `#[tauri::command(rename_all = "camelCase")]` 函数，耗时操作包 `spawn_blocking`，再到 `lib.rs` 的 `generate_handler!` 注册。
3. **补契约**：`src/lib/api.ts` 加类型与方法；`src/lib/devFallback.ts` 同步实现或明确抛错。
4. **做界面**：新建 `src/features/XxxTool.tsx`，在 `registry.ts` 登记 `id / name / desc / group / icon / keywords`，图标加到 `components/icons.tsx`。
5. **过门禁**：`pnpm typecheck && pnpm test:core`。

## 隐私与安全

- 除你在「网络工具」中主动发起的探测与请求外，应用不产生任何网络流量；没有统计、崩溃上报、更新检查。
- 加解密与密钥生成在本地 Rust 侧完成，密钥只在内存中经由进程间调用传递，不写磁盘、不外发。
- HTTP 请求历史仅保存在本机 WebView 的 `localStorage`，可随时在「最近 → 清空」中删除。
- JWT 工具只做 Base64 解码与时间解析，**不验证签名**，不要用它判断令牌是否有效。
- AES / RSA / 哈希工具面向调试与联调，不是密钥管理系统；生产密钥请使用专用工具保管。

## 已知限制

- 仅 macOS 安装包由仓库维护者构建；Windows 产物需在 Windows 上自行构建（见上文）。
- IP 归属地只内置 IPv4 库，IPv6 会明确提示未内置（v6 库体积约 30 MB+，暂不引入）。
- HTTP 工具暂不支持文件上传（multipart 仅文本字段）、Cookie 管理与请求集合保存。
- 时区支持固定偏移（`+08:00` / `Z` 这类），不处理夏令时规则库。

## Roadmap

- [ ] 进制转换、文本 diff、大小写/排序/去重文本工具箱
- [ ] YAML / TOML / CSV ↔ JSON，JSON → 各语言类型定义
- [ ] JWT 签发验签、证书与 CSR 解析、TOTP
- [ ] 请求集合与多标签、拖拽文件直接打开
- [ ] 把 `ikt-core` 暴露为 CLI，供脚本与 CI 使用

## 贡献

欢迎 issue 与 PR。提交前请确保：

- `pnpm typecheck` 与 `pnpm test:core` 全部通过；
- 新能力优先落在 `ikt-core`，并带单元测试与契约测试；
- UI 文案保持克制：只放控件标签、计算结果与状态，不写说明性长句；
- 一个 PR 只做一件事，描述里说清「改了什么、怎么验证」。

## 许可证

[MIT](./LICENSE) © iKunTools contributors

内置的 `src-tauri/ip2region_v4.xdb` 来自 [ip2region](https://github.com/lionsoul2014/ip2region)，上游为 Apache-2.0 与 MIT 双许可（`Apache-2.0 OR MIT`），本项目按 MIT 条款使用。
