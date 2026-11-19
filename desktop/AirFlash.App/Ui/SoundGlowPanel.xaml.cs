using System.Text;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using AirFlash.App.Services;
using AirFlash.App.ViewModels;
using AirFlash.Core;
using Microsoft.Win32;

namespace AirFlash.App.Ui;

public partial class SoundGlowPanel : UserControl, IDisposable
{
    public static readonly DependencyProperty AppProperty = DependencyProperty.Register(
        nameof(App), typeof(AppViewModel), typeof(SoundGlowPanel),
        new PropertyMetadata(null, (sender, args) => ((SoundGlowPanel)sender).SetApp(args.NewValue as AppViewModel)));
    private AppViewModel? _app;
    private Receiver? _receiver;
    private AppSettings? _baseline;
    private int _hardwareVolume;
    private string? _testCase;
    private int _trial;
    private bool _busy, _disposed, _ready, _invalid, _testAudio;
    private string _negotiated = "";
    private readonly StringBuilder _record = new();

    public AppViewModel? App { get => (AppViewModel?)GetValue(AppProperty); set => SetValue(AppProperty, value); }
    public SoundGlowPanel() { InitializeComponent(); }
    private void SetApp(AppViewModel? app)
    {
        if (_disposed || ReferenceEquals(_app, app)) return;
        if (_app is not null) { _app.Session.SoundGlowFeedback -= OnFeedback; _app.Session.Changed -= OnSessionChanged; }
        _app = app;
        if (_app is not null) { _app.Session.SoundGlowFeedback += OnFeedback; _app.Session.Changed += OnSessionChanged; }
        UpdateState();
    }
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_app is not null) { _app.Session.SoundGlowFeedback -= OnFeedback; _app.Session.Changed -= OnSessionChanged; _app = null; }
    }
    private async void OnCaseClick(object sender, RoutedEventArgs args)
    {
        if (_busy || _disposed || _app is null || sender is not Button { Tag: string testCase }) return;
        _busy = true;
        try
        {
            if (_baseline is null)
            {
                var snapshot = _app.Session.Snapshot;
                var receiver = snapshot.Receiver ?? throw new InvalidOperationException("请先在主界面连接要测试的 HomePod。");
                if (snapshot.DeviceVolume?.Actual is not > 0 || !snapshot.DeviceVolume.Available)
                    throw new InvalidOperationException("等待读取 HomePod 音量，或先将设备取消静音。");
                if (_app.Session.Muted) throw new InvalidOperationException("请先取消 AirFlash 静音。");
                _hardwareVolume = snapshot.DeviceVolume.Actual.Value;
                _receiver = receiver;
                _baseline = _app.Settings.Clone();
                _testAudio = SourceChoice.SelectedIndex == 0;
                Append($"锁定：{receiver.Name} / {receiver.Id}；HomePod 音量 {_hardwareVolume}；PCM 增益 {_baseline.Gain(receiver.Id):F3}；{(_testAudio ? "内置音频" : "当前采集设置")}；ALAC 44100 Hz；PTP 发送端主时钟；请求提前量 1000 ms。");
            }
            var settings = SoundGlowComparison.Create(_baseline, _receiver!, testCase, _testAudio, _hardwareVolume);
            if (_testCase is not null) SaveObservation();
            // Stop the old experiment before tagging subsequent feedback with the
            // new trial. StartAsync itself also enforces full session teardown.
            await _app.Session.StopAsync();
            _testCase = testCase; _trial++; _ready = false; _invalid = false; _negotiated = "";
            TrialText.Text = $"第 {_trial} 轮 / {testCase}：连接中";
            Observation.SelectedIndex = 0; Notes.Text = "";
            Append($"BEGIN trial={_trial} case={testCase} media_remote={settings.GlowComparison!.MediaRemote} transport={settings.ReadOptions(_receiver!.Id).Transport} source={(_testAudio ? "glow_test" : _baseline.CaptureMode)}");
            UpdateState();
            await _app.Session.StartAsync(_receiver, settings);
        }
        catch (Exception error) { _invalid = true; Append(error.Message); }
        finally { _busy = false; UpdateState(); }
    }
    private async void OnStopClick(object sender, RoutedEventArgs args)
    {
        if (_disposed || _app is null || _busy) return;
        _busy = true; UpdateState();
        try { await _app.Session.StopAsync(); Append($"STOP trial={_trial}"); }
        catch (Exception error) { Append(error.Message); }
        finally { _busy = false; UpdateState(); }
    }
    private async void OnResetClick(object sender, RoutedEventArgs args)
    {
        if (_busy || _disposed || _app is null) return;
        _busy = true; UpdateState();
        try
        {
            var receiver = _receiver ?? _app.Session.Snapshot.Receiver;
            await _app.Session.StopAsync();
            _baseline = null; _receiver = null; _testCase = null; _ready = false; _invalid = false; _negotiated = "";
            TrialText.Text = "已恢复普通播放，下一轮重新锁定条件";
            if (receiver is not null) await _app.Session.StartAsync(receiver, _app.Settings.Clone());
            Append("恢复普通播放；HomePod 可能暂时保留上一张封面。");
        }
        catch (Exception error) { Append(error.Message); }
        finally { _busy = false; UpdateState(); }
    }
    private void OnFeedback(JsonElement item)
    {
        var trial = _trial;
        Dispatcher.BeginInvoke(() =>
        {
        if (_disposed) return;
        if (trial != _trial) return;
        var kind = item.Text("event");
        Append($"trial={_trial} case={_testCase} {item.GetRawText()}");
        if (item.Boolean("accepted") == false || kind == "sound_glow_error") _invalid = true;
        if (kind == "sound_glow_protocol_ready")
        {
            _ready = item.Boolean("accepted") == true && !_invalid;
            TrialText.Text = _ready ? $"第 {_trial} 轮 / {_testCase}：初始化已接受，请观察原生灯效" : $"第 {_trial} 轮 / {_testCase}：初始化失败，本轮无效";
        }
        UpdateState();
        });
    }
    private void OnSessionChanged(SessionSnapshot snapshot) => Dispatcher.BeginInvoke(() =>
    {
        if (_disposed) return;
        if (_testCase is not null && snapshot.State is PlaybackState.Error or PlaybackState.Preempted)
        {
            _invalid = true;
            TrialText.Text = $"第 {_trial} 轮 / {_testCase}：连接失败，本轮无效";
            Append(snapshot.Message);
        }
        if (_testCase is not null && snapshot.Diagnostics?.Negotiated is { Count: > 0 } items)
        {
            var negotiated = string.Join(Environment.NewLine, items.Select(i => i.GetRawText()));
            if (negotiated != _negotiated) { _negotiated = negotiated; Append("实际协商：" + negotiated); }
            NegotiatedText.Text = "实际协商：" + string.Join("；", items.Select(i => $"{i.Text("host")} / {i.Text("codec")} / {i.Text("transport")} / {i.Text("timing")} / {i.Integer("sample_rate")} Hz / {i.Number("effective_latency_ms"):F0} ms"));
        }
        if (_testCase is not null && snapshot.State == PlaybackState.Streaming && !_ready && !_invalid)
            TrialText.Text = $"第 {_trial} 轮 / {_testCase}：音频已连接，等待封面和状态写入…";
        if (_ready && !_invalid && snapshot.State == PlaybackState.Streaming
            && (snapshot.DeviceVolume is { Status: "confirmed", Actual: int volume } && volume != _hardwareVolume || _app?.Session.Muted == true))
        {
            _invalid = true;
            TrialText.Text = $"第 {_trial} 轮 / {_testCase}：音量或静音条件已改变，本轮无效";
            Append("音量或静音条件改变，请重置后复测。");
        }
        UpdateState();
    });
    private void OnSaveObservation(object sender, RoutedEventArgs args) => SaveObservation();
    private void SaveObservation()
    {
        if (_testCase is null) { Append("请先启动一组测试。"); return; }
        var result = (Observation.SelectedItem as ComboBoxItem)?.Content?.ToString();
        Append("OBSERVATION " + JsonSerializer.Serialize(new { trial = _trial, test_case = _testCase,
            initialized = _ready, invalid = _invalid, observation = result, notes = Notes.Text,
            homepod_volume = _hardwareVolume, source = _testAudio ? "glow_test" : "capture" }));
    }
    private void OnExportClick(object sender, RoutedEventArgs args)
    {
        var dialog = new SaveFileDialog { Filter = "文本记录 (*.txt)|*.txt", FileName = $"SoundGlow-Protocol-{DateTime.Now:yyyyMMdd-HHmmss}.txt" };
        if (dialog.ShowDialog() != true) return;
        try { File.WriteAllText(dialog.FileName, _record.ToString(), Encoding.UTF8); Append("已导出：" + dialog.FileName); }
        catch (Exception error) { Append(error.Message); }
    }
    private void OnOpenLogsClick(object sender, RoutedEventArgs args)
    {
        try { AppPaths.OpenLogs(); }
        catch (Exception error) { Append(error.Message); }
    }
    private void UpdateState()
    {
        if (_disposed) return;
        var snapshot = _app?.Session.Snapshot;
        StateText.Text = _baseline is null
            ? "先连接设备；首次点击会锁定设备、音频来源和音量。"
            : $"固定设备：{_receiver!.Name} · HomePod {_hardwareVolume}% · {(_testAudio ? "内置音频" : "电脑音频")} · 每组重新连接";
        Actions.IsEnabled = !_busy && (_baseline is not null || snapshot is { State: PlaybackState.Streaming, DeviceVolume: { Available: true, Actual: > 0 } });
        SourceChoice.IsEnabled = !_busy && _baseline is null;
        StopButton.IsEnabled = !_busy && snapshot?.IsActive == true;
        ResetButton.IsEnabled = !_busy;
        if (_testCase is null) NegotiatedText.Text = "";
    }
    private void Append(string message)
    {
        if (_disposed) return;
        var line = $"{DateTime.Now:O} {message}";
        _record.AppendLine(line);
        AppPaths.Log("SoundGlow comparison: " + message);
        if (SoundGlowLog.Text.Length > 24000) SoundGlowLog.Text = SoundGlowLog.Text[^12000..];
        SoundGlowLog.AppendText(line + Environment.NewLine); SoundGlowLog.ScrollToEnd();
    }
}
