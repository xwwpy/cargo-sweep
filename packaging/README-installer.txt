Cargo Sweep 安装说明
====================

请双击 INSTALL-CARGO-SWEEP.cmd，不要直接双击 setup.exe。

为什么需要这个脚本？
部分 Windows 环境的 TEMP / TMP 指向 NSIS 安装器无法写入的目录（例如 E:\Temp），
直接双击安装包会提示：

    NSIS Error
    Error writing temporary file. Make sure your temp folder is valid.

脚本会把本次安装使用的临时目录切换到本文件夹下的 temp 子目录，再启动安装程序。
安装完成后脚本会自动清理该临时目录，可以删除整个解压文件夹。

如果仍然失败：
1. 解压路径请放在可写目录（例如 Downloads），不要放在只读目录。
2. 可改用同版本的免安装版，解压后双击 START-CARGO-SWEEP.cmd。
3. 安装包没有代码签名，Windows 可能提示“未知发布者”，请选择“更多信息 → 仍要运行”。

系统要求：Windows 10/11 x64，WebView2 Runtime。

安装完成后，应用内可自动检查 GitHub Releases 更新。
