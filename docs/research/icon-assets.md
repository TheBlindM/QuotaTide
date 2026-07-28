# QuotaTide 应用图标与托盘资产

> 状态：v1 实施基线
>
> 日期：2026-07-28
>
> 选择：A — Tide Dial（潮汐仪表）

## 视觉决定

用户从三方向原型中选择 A。最终标记由“圆形额度仪表 + 上升潮水”组成：

- 圆环表示当前账号的额度窗口；
- 七个刻度表示账号当前额度周期中的七天，而不是滚动最近七天；
- 潮水表示额度消耗、水位与重置后的回落；
- 不使用字母、产品名、OpenAI/Codex 标记或相似的结形图案。

正式资产不是对概念位图的描摹。所有生产源文件均为仓库内手工构建的原创
SVG，并作为 MIT 项目资产分发。

## 光学校正

应用图标不是把一个 1024 px 图机械缩放到所有尺寸：

| 使用尺寸 | 源文件 | 保留内容 |
|---|---|---|
| 128–1024 px | `assets/branding/app-icon.svg` | 毛玻璃层次、七刻度、双层仪表环、潮水高光 |
| 16–64 px | `assets/branding/app-icon-small.svg` | 深色底、粗仪表环、单一潮水轮廓 |
| 系统托盘 | `assets/branding/tray-*.svg` | 仪表外环与实心潮水，不含刻度和装饰高光 |

32 px 及以下删除刻度是有意的光学校正，不属于功能或品牌语义丢失。托盘图标
必须优先在 16–22 px 下形成稳定轮廓。

## 应用图标输出

Tauri 配置使用：

```json
{
  "bundle": {
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

`src-tauri/icons` 包含：

| 文件 | 内容 |
|---|---|
| `32x32.png` | 32 px RGBA 光学校正版 |
| `128x128.png` | 128 px RGBA |
| `128x128@2x.png` | 256 px RGBA |
| `icon.png` | 512 px RGBA 通用源 |
| `icon.icns` | macOS 16–1024 px 完整 iconset |
| `icon.ico` | Windows 32、16、24、48、64、256 px；32 px 层位于首位 |

ICO 层顺序遵循 Tauri 对开发期显示的建议。PNG 均为正方形、8-bit RGBA。

## 托盘状态

### macOS

- 默认路径：`icons/tray/tray-template.png`（22 px）；
- Retina 资产：`icons/tray/tray-template@2x.png`（44 px）；
- Tauri 设置 `iconAsTemplate: true`；
- 源图使用黑色像素与透明背景，由 macOS 自动适配浅色、深色与系统强调状态；
- 不把彩色 Windows 图标作为 macOS menu bar 图标。

### Windows

- 默认路径：`icons/tray/tray-windows.ico`；
- ICO 层：32、16、20、24、48、64 px，32 px 在首位；
- 默认图标使用深色圆底与青蓝仪表，确保浅色和深色任务栏都有边界；
- 高对比浅底使用 `tray-contrast-dark.ico`；
- 高对比深底使用 `tray-contrast-light.ico`；
- 主题/高对比切换由后续平台 adapter 原子替换 tray icon，不能重新创建第二个
  tray 实例。

`tray-template-inverse*.png` 用于运行时或视觉测试，不用于 macOS template
模式的默认路径。

## 生成与验证

在 macOS 上运行：

```sh
npm run icons
```

生成器使用系统 `sips` 输出 RGBA PNG、使用 `iconutil` 生成 ICNS，并用项目内
ICO writer 生成多层 Windows 图标。输出已验证：

- PNG 尺寸、8-bit 深度与 RGBA color type；
- Windows app ICO 包含 16、24、32、48、64、256 px，且 32 px 为首层；
- Windows tray ICO 包含 16、20、24、32、48、64 px，且 32 px 为首层；
- ICNS 可以由 `iconutil` 完整还原为十个标准 iconset 文件；
- 连续执行两次生成器得到逐字节一致的输出。

Windows 构建使用已经提交的生成资产，不要求在 Windows 上运行该 macOS
生成器。Rust/Tauri 工程建立后，CI 继续运行跨平台的静态格式测试；只有图形
源文件发生变化时才需要在 macOS 重新导出。

## 使用边界

- 不把中文副标题或 QuotaTide 文字烘焙到图标；
- 不改变仪表环与潮水的基本比例来表达实时百分比；托盘图标本身不是动态图表；
- 告警状态通过系统通知与窗口状态表达，不把常驻托盘图标整体改成红色；
- 不复用概念原型中的生成位图作为生产资产；
- 不使用 OpenAI、Codex 或其他第三方 logo。

## 官方接入依据

- [Tauri App Icons](https://v2.tauri.app/develop/icons/)：桌面 bundle 文件名、
  PNG/ICO 格式和 ICO layer 要求；
- [Tauri Tray API](https://v2.tauri.app/reference/javascript/api/namespacetray/)：
  macOS template 状态与运行时原子替换；
- [Tauri Configuration](https://v2.tauri.app/reference/config/)：
  `iconPath` 和 `iconAsTemplate` 配置。
