# Live2D 绑定指南（Cubism Editor 手工操作手册）

分层素材已就绪（`assets-src/live2d/comet_live2d.psd`，15 层），本文档指导在 Cubism Editor 中完成绑定并导出 Comet 可用的模型。预计首次操作 3~6 小时（含熟悉软件）。

## 0. 安装

1. 下载 [Live2D Cubism Editor](https://www.live2d.com/en/cubism/download/editor/)（macOS 版，免费试用 42 天 PRO，之后可转 FREE 版继续用）
2. FREE 版限制：参数最多 30 个、部件 100 个——本模型规模完全够用
3. 导出的 moc3 允许在自有应用中使用（Publication License：个人/小规模免费）

## 1. 导入 PSD

File → Open → 选择 `comet_live2d.psd`。确认 15 个图层完整：

| 图层（自顶向下） | 默认可见 | 用途 |
|---|---|---|
| nose | ✅ | 鼻子 |
| mouth_open / tongue | ❌ | 张嘴吐舌（与 mouth_closed 切换） |
| mouth_closed | ✅ | 闭嘴微笑 |
| eyes_closed | ❌ | 闭眼（眨眼用） |
| eye_right / eye_left | ✅ | 眼睛（眼珠跟随） |
| brows | ✅ | 眉毛 |
| head_base | ✅ | 头部基底（无五官） |
| ear_right / ear_left | ✅ | 垂耳（物理摆动） |
| leg_right / leg_left | ✅ | 前腿 |
| torso | ✅ | 身体 |
| tail | ✅ | 尾巴（物理摆动） |

## 2. 网格（Mesh）

全选所有部件 → Modeling → Texture → Auto Mesh Generator：
- 毛发部件（head_base/ears/torso/tail）用「Standard」密度
- 五官小部件用「Fine」
- 手动检查耳朵网格边缘要包住毛发轮廓（出血 5px 左右）

## 3. 变形器与参数（最小可用集）

创建以下参数（Editor 默认参数名保持一致，运行时会按标准 ID 驱动）：

| 参数 ID | 范围 | 绑定内容 |
|---|---|---|
| `ParamAngleX` | -30~30 | 头部组（head_base+五官+耳）左右转：整组套一个旋转变形器，两端关键帧微移+透视微缩 |
| `ParamAngleY` | -30~30 | 头部组上下点头 |
| `ParamAngleZ` | -30~30 | 头部组歪头（旋转） |
| `ParamEyeLOpen` / `ParamEyeROpen` | 0~1 | 1=eye_x 可见；0=对应侧 eyes_closed 可见（用 Glue 或不透明度关键帧切换） |
| `ParamEyeBallX` / `ParamEyeBallY` | -1~1 | 眼珠在眼眶内位移（眼珠层小幅移动） |
| `ParamMouthOpenY` | 0~1 | 0=mouth_closed；>0.3 渐变到 mouth_open+tongue（不透明度切换 + tongue 微升） |
| `ParamBodyAngleZ` | -10~10 | torso+四肢整体微倾 |
| `ParamBreath` | 0~1 | torso 纵向 2% 缩放 + head 微升降 |

操作套路都是一样的：选中部件/变形器 → 在参数条上打 2~3 个关键点 → 在每个关键点摆放该部件的形态。

## 4. 物理（耳朵/尾巴）

Modeling → Physics/Scene Blend Settings：
- 新建物理组 `Ears`：输入 `ParamAngleX/Z`，输出耳朵旋转变形器，摆锤 2 节，摇动幅度 15°，衰减 0.85
- 新建物理组 `Tail`：输入 `ParamBodyAngleZ` + `ParamBreath`，输出尾巴旋转，1 节即可

## 5. 导出

File → Export For Runtime → Export as moc3：
- 版本选 SDK 4.x / Cubism 4
- 勾选 physics3.json
- 导出目录：`comet/src/assets/live2d-model/`（包含 `comet.moc3`、`comet.model3.json`、`comet.physics3.json`、纹理 atlas）

## 6. 交回集成

导出文件放入上述目录后告诉 AI 助手，运行时集成（PixiJS + pixi-live2d-display、参数驱动：光标→眼珠/头角度、待机呼吸循环、随机眨眼、点击→吐舌）已规划好，代码侧会接管后续全部工作。

## 常见坑

- PSD 重新导入更新素材：File → Re-import PSD，图层名不要改
- 免费版导出的 moc3 完全可用，只是 Editor 内功能受限
- 参数 ID 务必用上表的标准名（`ParamAngleX` 等），运行时按标准 ID 驱动，改名会导致动画失效
