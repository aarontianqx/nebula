# Comet

极致轻量、低资源消耗的桌面宠物。以主人的泰迪为原型，常驻桌面但不打扰办公：透明无边框窗口 + 动态鼠标穿透，只有点击宠物本体才会触发交互，点空白处直接穿透到底层软件。

## Prerequisites

- Node.js ≥ 18 与 Yarn 1.x
- Rust stable 工具链（`rustup`）
- Tauri v2 系统依赖（macOS: Xcode CLT；Linux: webkit2gtk 等；见 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)）

## Commands

```bash
yarn install        # 安装前端依赖
yarn tauri:dev      # 开发模式（透明窗口 + 热更新）
yarn tauri:build    # 打包发布
yarn typecheck      # TypeScript 检查
yarn lint           # ESLint
cargo clippy -- -D warnings   # Rust lint（在 comet/ 下运行）
cargo fmt           # Rust 格式化
```

## Current state

Phase 1：透明置顶窗口、像素级动态鼠标穿透、16 格序列帧资产（皮克斯风泰迪）、待机姿势轮换、屏幕随机走动、拖拽（被拎起姿势）与点击（开心姿势）反馈、健康饮水提醒（到点宠物舔水碗，点击确认后欢呼重新计时）、番茄钟（双击开始/取消：专注期戴眼镜盯屏，完成欢呼，休息期趴卧）、系统状态联动（CPU 高负载或低电量时瘫倒吐舌）。右键宠物退出。

资产重刷：替换 `src/assets/pet/_source-grid.png` 后运行 `scripts/slice_grid.py`（需 pillow/numpy/scipy）。功能规格见 `specs/features/`。
