import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const rootPid = Number(process.argv[2]);
const durationSeconds = Number(process.argv[3] ?? 300);
const intervalSeconds = Number(process.argv[4] ?? 5);
assert.ok(Number.isInteger(rootPid) && rootPid > 1, "Pass the app process ID");
assert.ok(durationSeconds >= intervalSeconds && intervalSeconds >= 1);

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function snapshot() {
  const { stdout } = await execFileAsync(
    "ps",
    ["-axo", "pid=,ppid=,%cpu=,rss=,comm="],
    { encoding: "utf8" },
  );
  const processes = stdout
    .trim()
    .split("\n")
    .map((line) => {
      const match = line.trim().match(/^(\d+)\s+(\d+)\s+([\d.]+)\s+(\d+)\s+(.+)$/);
      return match
        ? {
            pid: Number(match[1]),
            parentPid: Number(match[2]),
            cpu: Number(match[3]),
            rssKiB: Number(match[4]),
            command: match[5],
          }
        : null;
    })
    .filter(Boolean);
  const selected = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const process of processes) {
      if (selected.has(process.parentPid) && !selected.has(process.pid)) {
        selected.add(process.pid);
        changed = true;
      }
    }
  }
  const tree = processes.filter((process) => selected.has(process.pid));
  assert.ok(tree.some((process) => process.pid === rootPid), "App process exited");
  return {
    cpuPercent: tree.reduce((sum, process) => sum + process.cpu, 0),
    rssMiB:
      tree.reduce((sum, process) => sum + process.rssKiB, 0) / 1024,
    processCount: tree.length,
    commands: [...new Set(tree.map((process) => process.command))].sort(),
  };
}

const sampleCount = Math.floor(durationSeconds / intervalSeconds) + 1;
const samples = [];
for (let index = 0; index < sampleCount; index += 1) {
  samples.push(await snapshot());
  if (index > 0 && index % Math.ceil(60 / intervalSeconds) === 0) {
    console.log(`resource sample ${index + 1}/${sampleCount}`);
  }
  if (index + 1 < sampleCount) await sleep(intervalSeconds * 1000);
}
const cpuAverage =
  samples.reduce((sum, sample) => sum + sample.cpuPercent, 0) / samples.length;
const memories = samples.map((sample) => sample.rssMiB).sort((a, b) => a - b);
const result = {
  rootPid,
  durationSeconds,
  intervalSeconds,
  sampleCount: samples.length,
  cpuAveragePercent: Number(cpuAverage.toFixed(3)),
  memoryMedianMiB: Number(memories[Math.floor(memories.length / 2)].toFixed(2)),
  memoryPeakMiB: Number(Math.max(...memories).toFixed(2)),
  processCountPeak: Math.max(...samples.map((sample) => sample.processCount)),
  commands: [...new Set(samples.flatMap((sample) => sample.commands))].sort(),
  preliminaryPass: {
    cpu: cpuAverage < 0.5,
    memory: Math.max(...memories) <= 180,
  },
};
console.log(JSON.stringify(result, null, 2));
