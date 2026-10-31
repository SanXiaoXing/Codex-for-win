# Win CodexBar v0.1.4

**发布日期：** 2026-10-30

## ✨ 新功能

### 托盘图标动态圆环进度
- 系统托盘悬浮球图标改为四段式圆环进度动态渲染，实时反映当前用量状态

### 悬浮球自动跟随前台应用
- 悬浮球模式下自动跟随前台应用切换页签，无需手动切换服务

---

# Win CodexBar v0.1.3

**发布日期：** 2026-10-26

## 🐛 修复

### 修复 AtomCode 用量同步失败
- 修复部分网络环境中访问 AtomGit IPv4 地址时出现 TLS 握手中断，导致 AtomCode 页面显示「同步异常」的问题
- 为 AtomCode 使用独立的网络客户端，DNS 同时返回 IPv6 和 IPv4 地址时优先尝试 IPv6，并保留 IPv4 连接回退
- 本机 DNS 解析失败、超时或无可用地址时，继续使用客户端默认的 DNS／代理流程，兼容由代理解析域名的环境
- 用量查询与每日签到共用修复后的客户端，恢复窗口用量、套餐有效期、用量明细及签到状态展示

## ✅ 验证
- 14 项单元测试通过
- 新增可手动运行的真实接口回归检查，仅查询状态和用量，不触发签到；默认跳过，避免依赖本地登录态和网络
- 真实接口回归、前端构建及 Windows 正式版构建通过，并在应用界面确认 AtomCode 已连接、签到成功

## 📝 文档
- README 补充 AtomCode IPv6 优先策略及真实接口回归检查命令

---

# Win CodexBar v0.1.2

**发布日期：** 2026-10-16

## ✨ 新功能

### 新增 AtomCode CodingPlan 用量监控
- 新增 AtomCode 服务页签，实时展示 CodingPlan 用量
- 读取 AtomCode CLI 本地登录态并调用 CodingPlan API
- 支持每日签到（claim-v2），含静默期与重试间隔控制
- 系统托盘悬浮球新增 AtomCode 剩余百分比
- 设置页新增 AtomCode 说明及「打开 CodingPlan 页面」快捷入口
- 支持补充指标（`UsageMetric` / `metrics`）展示

### 新增 TRAE 浏览器会话自动续期
- 设置面板新增 Cookie 粘贴输入框，自动换取会话
- 通过 `GetUserToken` 接口用 Cookie 换取 8 小时 JWT，实现自动续期
- JWT 缓存与过期检测，剩余不足 10 分钟时自动重新换取
- 旧 `Authorization` 方式折叠为备用选项
- 新增 base64url 解码与 JWT `exp` 解析工具函数（含单元测试）

## 🛠 改进
- 页签切换逻辑重构为从 tabs 数组动态生成，现支持 4 个服务（Codex / WorkBuddy / TRAE / AtomCode）
- `ProviderUsage` 模型新增 `metrics` 字段，各 provider 适配
- 窗口尺寸适配 4 列页签布局
- TRAE base64 解码改用 `base64` crate，移除手写实现
- 提取 `open_url` 公共函数

## 📝 文档
- README 更新：TRAE 推荐通过 Cookie 粘贴配置，而非手动重贴 Authorization

---

# Win CodexBar v0.1.1

**发布日期：** 2026-10-15

## ✨ 新功能
- 首个可用版本：Windows 系统托盘用量监控应用，支持 Codex / WorkBuddy / TRAE 三个服务
- 托盘悬浮球实时显示用量，窗口内分 Tab 展示各服务详情
- 设置面板管理各服务凭据
- 右键上下文菜单：刷新、切换服务、设置、收起悬浮球
- 键盘快捷键：`Ctrl+R` 刷新、`Ctrl+1/2` 切换服务、`Ctrl+,` 打开设置
- 无障碍支持：焦点陷阱与 ARIA 属性
- 「减少动态效果」与「减少透明度」偏好选项
- Tab 改用 SVG 图标与滑动选中样式
- 非 Tauri 环境可降级运行（`isTauri()` 判断）

## 🛠 改进
- 全局配色、字体栈及玻璃拟态背景重做
- 产品名称统一为 **Win CodexBar**，包名移除 `mvp` 后缀
- TRAE 用量面板与托盘 tooltip 默认显示**剩余积分**而非已用积分
- 移除旧版 TRAE 签到模块
- 代码模块化重构，安全错误体处理提取至共享模块
- `windows` 依赖限定为仅 Windows 平台编译
