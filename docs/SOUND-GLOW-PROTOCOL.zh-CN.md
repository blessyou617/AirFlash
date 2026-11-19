# AirFlash Sound Glow 协议对照 v1

这是基于 AirFlash 0.7.0 源码的 Windows x64 独立测试版。目标是检验 MediaRemote 播放状态和音频传输类型是否影响 HomePod 的原生律动。结果需要在真实 HomePod 上观察，代码通过测试不等于灯效已经成功。

## 开始测试

1. 完全退出旧版 AirFlash（包括托盘），解压本包，运行 AirFlash.exe。
2. 在家庭 App 中开启 HomePod 的 Sound Glow。先在主界面连接目标设备，选择舒适的音量；初版支持单个 HomePod 或苹果原生立体声组。
3. 打开设置中的 Sound Glow 实验页。页面标题应是 **Sound Glow · 协议对照 v1**。
4. 保持默认“内置音频”，点击 A。第一次点击会锁定设备、音频来源、硬件音量和 PCM 增益。测试使用发送端 PTP 主时钟。
5. 等待显示“初始化已接受”，观察至少一个完整的 64 秒循环。选择观察结果并点“保存本轮观察”。
6. 按 A→B→A，再 C→D→C 的顺序测试。每个按钮都建立新会话、从头播放同一音频。
7. 导出完整测试记录，连同你看到的灯效发回来。记录包含协议响应、真实协商模式、音量、固定封面 SHA-256 和你的观察。

| 按钮 | 真实音频传输 | MediaRemote |
|---|---|---|
| A | realtime，type 96，UDP | 关闭，仅 DMAP 歌曲信息＋封面 |
| B | realtime，type 96，UDP | 开启，额外发送原生播放状态 |
| C | buffered，type 103，TCP | 关闭，仅 DMAP 歌曲信息＋封面 |
| D | buffered，type 103，TCP | 开启，额外发送原生播放状态 |

先比较 A/B：同一传输模式，只改变 MediaRemote；再比较 C/D。
比较 A/C、B/D：同一 MediaRemote 条件，只改变传输模式。编码固定 ALAC，采样率固定 44100 Hz，EQ 和静音待机关闭，提前量均请求 1000 ms。接收端若要求更大的最小提前量，实际值会显示在协商记录中；出现不同值时不能声称所有时序参数一致。这里的“缓冲”是 type 103 传输，不是单纯把 UI 延迟调高。

内置测试音每 64 秒重复：0–8 秒静音，8–24 秒 120 BPM 强弱鼓点，24–40 秒鼓点＋合成旋律，40–56 秒稳定的 220 Hz 音，56–64 秒静音。稳定音不是音乐效果的阳性对照，而是帮助区分“随机持续动画”和“与节拍相关的变化”。它也不能取代真实音乐，所以完成内置测试后请点“恢复普通播放 / 重置对照”，选择电脑音频再测一次，每组手动从同一位置重播同一首歌。

四组使用同一张 512×512 JPEG：粉、紫、青、黑四条竖带，黑色占 25%。封面与音量强弱不挂钩，播放期间不会周期性更新封面或元数据。页面隐藏后固定试验仍继续，点击“停止测试”才结束；“恢复普通播放”重新连接并解除固定输入。

## 如何判断结果

- B 比 A 明显跟拍，且第二次 A 又消失：支持 MediaRemote 状态是关键因素，但仍需重复确认。
- D 比 B 明显跟拍，A/C 也有相同趋势：音频传输模式值得进一步调查。
- B/D 的请求出现 400、415、超时等错误：本轮无效，不能解释为“MediaRemote 对灯光没用”。失败阶段会单独记录。
- 全部 200：仅表示 HomePod 接受请求，不能证明启用了某个内部灯效算法。
- 第二次 A 与第一次 A 不一样：可能存在 HomePod 状态残留。先停止播放、确认其他手机或电脑没有占用设备，再复测；仍不一致时记录此现象，必要时重启 HomePod。
- 静音时有动画，或者动画始终按自身周期变化：不要仅凭“动了”记作跟拍成功。
- 初次封面切换、曲目信息出现时的单次闪动不计入律动。

比较期间不要调音量、EQ、采集来源、其他设备设置，也不要用 HomePod 顶部或手机遥控暂停/切歌。当前仅对齐发送端状态公告；它不是完整的 MediaRemote 双向播放器。若条件改变，请重置对照重新锁定。

## 本版实际对齐了什么

参考固定版本：
https://github.com/music-assistant/airplay-cli/tree/0a58c239d5fb3cf8927b9707a71c6fe897266b6a

开启 MR 时，在既有 HAP 加密 RTSP 连接上向 `/command` 顺序发送：

1. DEVICE_INFO：带 varint 长度前缀的 ProtocolMessage protobuf，包装在二进制 plist 的 params.data 中。
2. updateMRNowPlayingInfo：npi-text / replace，包含音乐媒体类型、播放速率、日期、相同封面字节和稳定的曲目身份。
3. updateMRSupportedCommands。
4. updateMRPlaybackState：Playing。
5. updateMRNowPlayingClient：裸 NowPlayingClient protobuf。

DEVICE_INFO 的 routingContextID / airPlayGroupID 与本次 SETUP 的 sessionUUID / groupUUID 相同。音频本身仍走 AirFlash 引擎。没有开启另一个 type 130 通道；参考 airplay-cli 的默认状态发送路径也是 `/command`。

这是针对参考实现中可隔离状态通道的移植，不代表整个 airplay-cli 已移植到 Windows。仍可能有 SETUP、事件通道、延迟、发送节奏或接收端固件条件尚未对齐。

## 可选：真正的 airplay-cli 外部对照

包内 `reference/` 含从同一 Rust 函数导出的 `protocol-test.wav`、完全相同的 `protocol.jpg` 和 `sound_glow_reference.py`。脚本供 macOS/Linux 使用，需要你已安装上述固定版本的 cliairplay 和 Python 3。它调用真实 cliairplay，可分别设置 MR 开关和 realtime/buffered。请先退出 AirFlash，避免占用同一设备和 PTP 端口。

从 `reference` 目录运行：

```sh
sudo python3 sound_glow_reference.py --binary /完整路径/cliairplay-macos-arm64 --host 192.168.1.9 --txt-file homepod-txt.txt --volume 25 --gain 1 --mrp off --transport realtime
```

`homepod-txt.txt` 放设备真实的 `_airplay._tcp` TXT 内容（包含 model 等字段）；在 Mac 上可用 `dns-sd -L '设备名称' _airplay._tcp local` 查看。不要使用猜测的型号或功能位。`--volume` 必须匹配 Windows 记录的 HomePod 音量；`--gain` 匹配日志里的 PCM 增益。再用 `--mrp on`、`--transport buffered` 逐项比较。

脚本会等 PTP 和音频就绪后起播，发送固定歌曲信息及封面，保存 CLI 原始日志。需要占用 UDP 319/320，故示例用 sudo；只启动/停止自己的子进程。此参考脚本没有在你的 HomePod 上实测。若在 Mac 与 Windows 间比较，发送主机与操作系统也变了，只能作为额外线索，不能把差异全部归因于 MediaRemote。

## 源码与构建

`Source.zip` 是对应此可执行文件的完整项目源码；`build-info.json` 给出构建提交和哈希，`SHA256SUMS.txt` 给出包内校验值。实验构建不发布上游正式版本。

Windows 构建顺序见 `.github/workflows/sound-glow-protocol.yml`：Rust 测试／lint／静态 CRT 构建，.NET 测试，独立打包，启动自检和兼容性自检。原生灯效由 HomePod 决定，没有以真人测试结果作任何保证。
