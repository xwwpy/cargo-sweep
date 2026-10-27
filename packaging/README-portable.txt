Cargo Sweep 免安装版
====================

请双击 START-CARGO-SWEEP.cmd 启动，不要直接双击 cargo-sweep.exe。

启动脚本会：
- 把 TEMP / TMP 指向程序目录下的 temp 文件夹，避开系统临时目录问题；
- 把 WebView2 数据放在程序目录下的 webview-data 文件夹；
- 启动 Cargo Sweep。

如果窗口没有出现，请查看：
%LOCALAPPDATA%\CargoSweep\logs\startup.log

系统要求：Windows 10/11 x64，WebView2 Runtime。
