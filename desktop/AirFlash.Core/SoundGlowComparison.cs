namespace AirFlash.Core;

/// <summary>Session-only inputs; never persisted in the user's settings.</summary>
public sealed record SoundGlowComparison(bool MediaRemote, bool TestAudio, string Case)
{
    public static AppSettings Create(AppSettings baseline, Receiver receiver, string testCase, bool testAudio, int hardwareVolume)
    {
        if (testCase is not ("A" or "B" or "C" or "D")) throw new ArgumentException("Unknown comparison case.", nameof(testCase));
        if (receiver.IsSoftwareGroup) throw new InvalidOperationException("首版对照测试请使用单个 HomePod 或苹果原生立体声组。");
        if (hardwareVolume is <= 0 or > 100) throw new InvalidOperationException("请先设置一个可听见的 HomePod 音量，再开始对照。");
        var settings = baseline.Clone();
        settings.GlowComparison = new(testCase is "B" or "D", testAudio, testCase);
        settings.Equalizer = new();
        settings.StandbyEnabled = false;
        settings.ForceLatency = false;
        settings.LatencyMode = "custom";
        settings.CustomBufferMs = 1000;
        if (testAudio) settings.CaptureMode = "loopback";
        foreach (var id in receiver.Peers.Select(p => p.Id).Append(receiver.Id).Distinct())
        {
            var options = settings.Options(id);
            options.Codec = "alac";
            options.SampleRate = "44100";
            options.Transport = testCase is "C" or "D" ? "buffered" : "realtime";
            // Preserve the sender-master PTP mode already proven on this device.
            options.Timing = "ptp";
            options.LatencyMode = "custom";
            options.CustomBufferMs = 1000;
            options.ForceLatency = false;
            options.LastVolume = hardwareVolume;
        }
        return settings;
    }
}
