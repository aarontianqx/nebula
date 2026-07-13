# Comet

极致轻量、低资源消耗的桌面宠物。以主人的泰迪为原型，常驻桌面但不打扰办公：透明无边框窗口 + 动态鼠标穿透，只有点击宠物本体才会触发交互，点空白处直接穿透到底层软件。

## Prerequisites

- Node.js ≥ 18 与 Yarn 1.x
- Rust stable 工具链（`rustup`）
- Tauri v2 系统依赖（macOS: Xcode CLT；Linux: webkit2gtk 等；见 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)）
- 资产管线（可选，仅重刷资产时需要）：Python 3.9+ 与 `pillow numpy scipy`，系统安装 `pngquant`

## Commands

```bash
yarn install        # 安装前端依赖
yarn tauri:dev      # 开发模式（透明窗口 + 热更新）
yarn tauri:build    # 打包发布
yarn typecheck      # TypeScript 检查
yarn lint           # ESLint
cargo clippy -- -D warnings   # Rust lint（在 comet/ 下运行）
cargo fmt           # Rust 格式化

# 资产管线（重生成宫格图后）
python scripts/slice_frames.py     # 切片 + 压缩 → src/assets/pet/frames/
python scripts/preview_frames.py   # 生成目检拼图 → /tmp/comet_preview/
conda run -n search-rec python scripts/validate_animation.py  # GIF + 连贯性指标
```

## Current state

Phase 2：透明置顶窗口、像素级动态鼠标穿透、15 状态混合动画（标准姿势 + 程序微动画 / 语义关键帧 / 连续序列）、待机姿势轮换、屏幕随机走动与奔跑、拖拽与点击反馈、健康饮水提醒、番茄钟和系统状态联动。右键宠物退出。

旧序列重刷：替换 `assets-src/frames/{state}_grid_4x4.png` 后运行 `scripts/slice_frames.py`。新关键姿势必须基于原狗 reference 生成，并用 `scripts/validate_animation.py` 检查真实播放连贯性。架构与资产规格见 `AGENTS.md` 与 `specs/features/`。
