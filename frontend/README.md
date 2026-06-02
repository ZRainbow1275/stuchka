# stuchka

Stučka 劳动纠纷智能体 · Flutter 桌面工作台外壳（通道 B IPC + 三栏工作台 + 无障碍底线）。

## 构建 Windows 桌面应用（Cyrillic 路径约束 + 跨机构建脚手架）

### 为什么需要构建脚手架

本仓库根路径含一个西里尔字母 c-with-caron（`...\Stučka\...`），且按产品约束 **必须保留在该路径**。
Flutter Windows 工具链中的原生着色器编译器 `impellerc` 无法把输出写入含非 ASCII 字符的构建路径，
因此 `flutter build windows` 仅会在着色器编译步骤失败 —— 这与 Dart 代码本身无关
（`app.dill` 可完整编译；`impellerc` 写入 ASCII 路径时退出码为 0）。

由于不能移动仓库，修复方式是一个可复现、跨机器的 **构建脚手架**，而非改代码或移仓库。

### 用法

```powershell
# 在 frontend/ 目录下（或任意目录，脚本会从自身位置推导工程根）
powershell -ExecutionPolicy Bypass -File tools/build_windows.ps1            # release（默认）
powershell -ExecutionPolicy Bypass -File tools/build_windows.ps1 -Mode debug
```

脚本 `tools/build_windows.ps1` 会：

1. 把工程源码（`lib/ test/ tools/ assets/ windows/ web/` + `pubspec.yaml` / `pubspec.lock` /
   `analysis_options.yaml` / `.metadata`）用 robocopy 镜像到 ASCII-only 工作目录
   （默认 `%TEMP%\stuchka_build`），并排除 `build/`、`.dart_tool/`、`ephemeral/` 等可重建缓存；
2. 在该 ASCII 目录里执行 `flutter pub get` 与 `flutter build windows --<release|debug>`
   （此处 `impellerc` 可正常写入着色器）；
3. 把产物 Runner 包（含 `stuchka.exe` 与 `data/`、`flutter_windows.dll` 等）拷回真实仓库的
   `frontend/build/windows-ascii-staged/`，并打印真实的 `flutter build windows` 退出码与 `.exe` 路径。

脚本是幂等的（重复运行会重新镜像源码并覆盖旧产物），且与路径无关
（从脚本自身位置推导工程根），因此在任意机器、任意检出路径下均可运行（跨机兼容）。
若你的 `%TEMP%` 本身含非 ASCII 字符，可用 `-StageRoot <ASCII 路径>` 覆盖暂存根目录。

### 已验证

`tools/build_windows.ps1 -Mode release` 实跑通过：`flutter build windows --release` 退出码 0，
产物为 `frontend/build/windows-ascii-staged/stuchka.exe`（PE32+ x86-64 GUI 可执行文件 + 完整 bundle）。

## 测试与门禁

```bash
flutter pub get
flutter analyze        # 0 issues
flutter test           # 全绿（含 no-emoji / no-forbidden-icons 门禁）
dart run tools/lint_no_emoji.dart .   # 零 Emoji 扫描
```
