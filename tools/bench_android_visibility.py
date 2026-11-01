#!/usr/bin/env python3
"""Check section connectivity and benchmark the actual optimized Rust module.

Synthetic sections only; this measures the connectivity stage, not game FPS.
No game, launcher, resources or player profile is opened.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    work = ROOT / '.local/android/visibility-benchmark'
    work.mkdir(parents=True, exist_ok=True)
    source = ROOT / 'crates/horizon-client-render/src/mesher/visgraph.rs'
    wrapper = work / 'probe.rs'
    wrapper.write_text(f'#[path = {json.dumps(source.as_posix())}] mod visgraph;\n', encoding='utf-8')
    binary = work / ('probe.exe' if os.name == 'nt' else 'probe')
    subprocess.run(['rustc', '--edition=2024', '--test', '-O', str(wrapper), '-o', str(binary)],
                   cwd=ROOT, check=True)
    result = subprocess.run([str(binary), '--include-ignored', '--nocapture', '--test-threads=1'],
                            cwd=ROOT, capture_output=True, text=True)
    print(result.stdout, end='')
    print(result.stderr, end='', file=sys.stderr)
    result.check_returncode()
    cases = [json.loads(line.split('VISGRAPH_BENCH ', 1)[1])
             for line in result.stdout.splitlines() if 'VISGRAPH_BENCH ' in line]
    if len(cases) != 8:
        raise RuntimeError('Incomplete visibility benchmark output')
    report = {
        'scope': 'Synthetic section connectivity, not whole-mesher time or device FPS',
        'rustc': subprocess.check_output(['rustc', '--version', '--verbose'], cwd=ROOT, text=True),
        'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'tracked_changes': bool(subprocess.check_output(['git', 'diff', 'HEAD', '--name-only'], cwd=ROOT)),
        'iterations_per_sample': 2048,
        'samples_per_case': 9,
        'cases': cases,
    }
    output = work / 'results.json'
    output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f'Report: {output.relative_to(ROOT)}')
    summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if summary:
        with Path(summary).open('a', encoding='utf-8') as stream:
            stream.write('Section connectivity benchmark (host CPU; not Android FPS).\n\n')
            stream.write('| Case | Before (µs) | After (µs) | Speedup |\n|---|---:|---:|---:|\n')
            for case in cases:
                stream.write(f'| {case["case"]} | {case["baseline_ns"] / 1000:.3f} '
                             f'| {case["optimized_ns"] / 1000:.3f} | {case["speedup"]:.2f}× |\n')


if __name__ == '__main__':
    main()
