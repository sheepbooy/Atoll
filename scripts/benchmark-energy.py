#!/usr/bin/env python3
"""macOS CPU comparison, with a temporary app and no hook/settings writes.

Build first: npm run build && cargo build --release --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml
Run: python3 scripts/benchmark-energy.py --scenes idle working
CPU percentages use one core as 100%; results are not watts.
The media scene uses the actual system player; start music before running it.
"""
import argparse
import ctypes
import json
import os
import pathlib
import plistlib
import shutil
import signal
import statistics
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]

def processes():
    output = subprocess.check_output(["ps", "-axo", "pid,comm"], text=True)
    return {int(line.split(None, 1)[0]): line.split(None, 1)[1] for line in output.splitlines()[1:] if line.strip()}

def run_scene(app, scene, mode, warmup, duration, lyrics_cache=None):
    before = processes()
    status_path = app.parent / f"{scene}-{mode}-status.json"
    launch = ["open", "-n", "--env", "ATOLL_BENCHMARK=1", "--env", f"ATOLL_BENCHMARK_SCENE={scene}",
                    "--env", f"ATOLL_BENCHMARK_MODE={mode}", str(app)]
    if scene == "media":
        launch[-1:-1] = ["--env", f"ATOLL_BENCHMARK_STATUS={status_path}"]
    if lyrics_cache:
        launch[-1:-1] = ["--env", f"ATOLL_BENCHMARK_LYRICS_CACHE={lyrics_cache}"]
    subprocess.run(launch, check=True)
    pid = None
    for _ in range(100):
        matches = [p for p, command in processes().items() if p not in before and str(app) in command and command.endswith("/atoll")]
        if matches:
            pid = matches[0]
            break
        time.sleep(0.1)
    if pid is None:
        raise RuntimeError("Benchmark app did not launch")
    try:
        print(f"{scene}/{mode}: warming up {warmup}s (pid {pid})", flush=True)
        time.sleep(warmup)
        if scene == "media" and (not status_path.exists() or not json.loads(status_path.read_text()).get("mediaAndLyricsReady")):
            raise RuntimeError("Music and matching lyrics were not ready; refusing an invalid media measurement")
        lib = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
        responsible = lib.responsibility_get_pid_responsible_for_pid
        responsible.argtypes = [ctypes.c_int]
        responsible.restype = ctypes.c_int
        owned = processes()
        webkit = [p for p, command in owned.items() if "WebKit" in command and responsible(p) == pid]
        adapters = [p for p, command in owned.items() if command.endswith("/perl") and responsible(p) == pid]
        pids = [pid, *webkit, *adapters]
        if len(pids) < 2:
            raise RuntimeError("Cannot attribute WebKit processes; refusing an incomplete CPU total")
        args = ["top", "-l", str(duration + 1), "-s", "1", "-stats", "pid,command,cpu"]
        for p in pids:
            args += ["-pid", str(p)]
        print(f"{scene}/{mode}: sampling {duration}s, attributed pids {pids}", flush=True)
        output = subprocess.check_output(args, text=True)
        samples = {p: [] for p in pids}
        for line in output.splitlines():
            fields = line.split()
            if fields and fields[0].isdigit() and int(fields[0]) in samples:
                samples[int(fields[0])].append(float(fields[2]))
        samples = {p: values[1:] for p, values in samples.items()}
        if any(len(v) != duration for v in samples.values()):
            raise RuntimeError("A process exited during sampling; rerun this scene")
        totals = [sum(v) for v in zip(*(samples[p] for p in [pid, *webkit]))]
        result = {"scene": scene, "mode": mode, "warmupSeconds": warmup, "sampleSeconds": duration,
                  "meanCpuPercent": round(statistics.mean(totals), 3), "minCpuPercent": min(totals),
                  "maxCpuPercent": max(totals), "adapterMeanCpuPercent": round(sum(statistics.mean(samples[p]) for p in adapters), 3), "processMeans": {str(p): round(statistics.mean(v), 3) for p, v in samples.items()}}
        print(json.dumps(result), flush=True)
        return result
    finally:
        try:
            rows = subprocess.check_output(["ps", "-axo", "pid=,ppid=,comm="], text=True)
            for row in rows.splitlines():
                fields = row.split(None, 2)
                if len(fields) == 3 and int(fields[1]) == pid and fields[2].endswith("/perl"):
                    try:
                        os.kill(int(fields[0]), signal.SIGTERM)
                    except ProcessLookupError:
                        pass
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenes", nargs="+", choices=["idle", "working", "media"], default=["idle", "working"])
    parser.add_argument("--modes", nargs="+", choices=["full", "auto"], default=["full", "auto"])
    parser.add_argument("--warmup", type=int, default=30)
    parser.add_argument("--duration", type=int, default=60)
    parser.add_argument("--lyrics-cache-dir", help="Isolated lyrics cache for a controlled media fixture")
    parser.add_argument("--output", default="/private/tmp/atoll-energy-benchmark.json")
    args = parser.parse_args()
    if args.duration < 1 or args.warmup < 0:
        parser.error("duration must be positive and warmup nonnegative")
    binary = ROOT / "src-tauri/target/release/atoll"
    if not binary.exists():
        parser.error("Build the release binary first")
    if "media" in args.scenes:
        adapter = ROOT / "src-tauri/resources/media"
        output = subprocess.check_output(["/usr/bin/perl", str(adapter / "mediaremote-adapter.pl"),
            str(adapter / "MediaRemoteAdapter.framework"), "get", "--now"], text=True)
        track = json.loads(output)
        if not track or not track.get("playing"):
            parser.error("Media scene needs an actual playing system media source; refusing an empty-media result")
    results = []
    with tempfile.TemporaryDirectory(prefix="atoll-energy-") as temp:
        app = pathlib.Path(temp) / "Atoll Benchmark.app"
        executable = app / "Contents/MacOS/atoll"
        executable.parent.mkdir(parents=True)
        shutil.copy2(binary, executable)
        with (app / "Contents/Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": "com.atoll.energy-benchmark", "CFBundleExecutable": "atoll",
                          "CFBundleName": "Atoll Benchmark", "CFBundlePackageType": "APPL", "LSUIElement": True,
                          "NSHighResolutionCapable": True}, file)
        for scene in args.scenes:
            for mode in args.modes:
                results.append(run_scene(app, scene, mode, args.warmup, args.duration, args.lyrics_cache_dir))
                pathlib.Path(args.output).write_text(json.dumps(results, indent=2) + "\n")
    print(f"Results: {args.output}", flush=True)

if __name__ == "__main__":
    main()
