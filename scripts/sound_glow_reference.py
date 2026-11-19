"""Compare the fixed inputs using an actual cliairplay binary on macOS/Linux."""

import argparse
import array
import hashlib
import math
import os
from pathlib import Path
import queue
import struct
import subprocess
import tempfile
import threading
import time
import wave


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--txt-file", type=Path, required=True)
    parser.add_argument("--volume", type=int, required=True)
    parser.add_argument("--gain", type=float, default=1.0)
    parser.add_argument("--mrp", choices=["off", "on"], required=True)
    parser.add_argument("--transport", choices=["realtime", "buffered"], required=True)
    parser.add_argument("--interface", help="Local IPv4 to bind, if needed")
    parser.add_argument("--seconds", type=int, default=70)
    parser.add_argument("--fixtures", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    if os.name == "nt":
        parser.error("This reference client needs POSIX FIFOs; run on macOS/Linux.")
    if not 1 <= args.volume <= 100 or not 0 < args.gain <= 1:
        parser.error("Use volume 1..100 and gain 0..1, matching the AirFlash log.")
    if not 64 <= args.seconds <= 600:
        parser.error("Use 64..600 seconds.")
    txt = args.txt_file.read_text().strip()
    if "model=" not in txt:
        parser.error("Supply the device's real _airplay._tcp TXT line including model=.")
    art = (args.fixtures / "protocol.jpg").resolve()
    with wave.open(str(args.fixtures / "protocol-test.wav"), "rb") as wav:
        if (wav.getnchannels(), wav.getframerate(), wav.getsampwidth()) != (2, 44100, 2):
            parser.error("Expected the included stereo 44100 Hz s16 WAV fixture.")
        pcm = wav.readframes(wav.getnframes())
    if args.gain != 1:
        values = array.array("h")
        values.frombytes(pcm)
        if struct.pack("=H", 1) != struct.pack("<H", 1):
            values.byteswap()
        gain = struct.unpack("f", struct.pack("f", args.gain))[0]
        for i, value in enumerate(values):
            product = struct.unpack("f", struct.pack("f", value * gain))[0]
            values[i] = int(math.copysign(math.floor(abs(product) + 0.5), product))
        if struct.pack("=H", 1) != struct.pack("<H", 1):
            values.byteswap()
        pcm = values.tobytes()
    stamp = time.strftime("%Y%m%d-%H%M%S")
    logfile = Path.cwd() / f"cliairplay-{stamp}-{args.transport}-mrp-{args.mrp}.log"
    stop = threading.Event()
    lines = queue.Queue()
    with tempfile.TemporaryDirectory(prefix="glow-reference-") as temporary, logfile.open("w") as log:
        fifo = Path(temporary) / "commands"
        os.mkfifo(fifo, 0o600)
        command_fd = os.open(fifo, os.O_RDWR | os.O_NONBLOCK)
        env = dict(os.environ, CLIAIRPLAY_MRP="1" if args.mrp == "on" else "0",
                   CLIAIRPLAY_BUFFERED="1" if args.transport == "buffered" else "0",
                   CLIAIRPLAY_PTP_FOLLOW="0")
        command = [str(Path(args.binary).resolve()), "--protocol", "airplay2", "--ap2-native",
                   "--ptp", "--port", "7000", "--latency", "1000", "--volume", str(args.volume),
                   "--samplerate", "44100", "--bitdepth", "16", "--channels", "2", "--debug", "5",
                   "--name", "AirFlash Protocol Test", "--txt", txt, "--cmdpipe", str(fifo)]
        if args.interface:
            command += ["--if", args.interface]
        command += [args.host]
        log.write(f"reference=0a58c239d5fb3cf8927b9707a71c6fe897266b6a (verify your binary source)\n")
        log.write(f"mrp={args.mrp} transport={args.transport} volume={args.volume} gain={args.gain}\n")
        log.write(f"pcm_sha256={hashlib.sha256(pcm).hexdigest()} artwork_sha256={hashlib.sha256(art.read_bytes()).hexdigest()}\n")
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env)

        def read_logs():
            for raw in iter(process.stdout.readline, b""):
                line = raw.decode("utf-8", errors="replace").rstrip()
                log.write(line + "\n")
                log.flush()
                print(line, flush=True)
                lines.put(line)

        def feed():
            try:
                while not stop.is_set():
                    for offset in range(0, len(pcm), 352 * 4):
                        if stop.is_set():
                            return
                        process.stdin.write(pcm[offset:offset + 352 * 4])
                        process.stdin.flush()
            except (BrokenPipeError, OSError):
                pass

        def send(text):
            os.write(command_fd, text.encode())

        logger = threading.Thread(target=read_logs, daemon=True)
        feeder = threading.Thread(target=feed, daemon=True)
        logger.start()
        feeder.start()
        try:
            clock_ready = audio_ready = False
            deadline = time.monotonic() + 35
            while not (clock_ready and audio_ready):
                if process.poll() is not None:
                    raise RuntimeError(f"cliairplay exited: {process.returncode}; inspect {logfile}")
                if time.monotonic() > deadline:
                    raise TimeoutError("PTP/audio not ready; trial invalid. See log.")
                try:
                    line = lines.get(timeout=0.2)
                except queue.Empty:
                    continue
                clock_ready |= "clock_ready mode=ptp state=ready" in line
                audio_ready |= "[STATUS] audio buffered_ms=" in line
            send("TITLE=AirFlash Sound Glow Protocol Test\nARTIST=AirFlash\nALBUM=Fixed Input Comparison v1\nITEMID=airflash-glow-protocol-v1\nSTART_UNIX_MS=0\nACTION=START\n")
            began = time.monotonic()
            time.sleep(1)
            send("ACTION=SENDMETA\n")
            time.sleep(2)
            send(f"ARTWORK={art}\n")
            print("Fixed inputs sent. Observe native lighting; no more artwork updates.", flush=True)
            while time.monotonic() - began < args.seconds:
                if process.poll() is not None:
                    raise RuntimeError("Client stopped before trial end; inspect log.")
                time.sleep(0.2)
        finally:
            stop.set()
            if process.poll() is None:
                try:
                    send("ACTION=DISCONNECT\n")
                    process.wait(timeout=5)
                except (OSError, subprocess.TimeoutExpired):
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            feeder.join(timeout=2)
            logger.join(timeout=2)
            os.close(command_fd)
    print(f"Log saved: {logfile}")


if __name__ == "__main__":
    main()
