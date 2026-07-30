export const REQUIRED_ENVIRONMENTS = {
  "M15-A": "macOS 15 latest patch / Apple Silicon",
  "M15-I": "macOS 15 latest patch / Intel",
  "MC-A": "Current stable macOS / Apple Silicon",
  "W25-X": "Windows 11 25H2 latest patch / x64",
  "W26-X": "Windows 11 26H1 latest patch / x64",
  "W25-B": "Windows 11 25H2 clean VM / x64 / WebView2 bootstrapper",
  "W25-F": "Windows 11 25H2 clean VM / x64 / bootstrapper failure",
  "W25-U": "Windows 11 25H2 / x64 / updated WebView2 first launch",
  "M14-C": "macOS 14 latest patch / compatibility only",
  "W10-C": "Windows 10 22H2 build 19045 / x64 / compatibility only",
  "W24-C": "Windows 11 24H2 / x64 / compatibility only",
};

const ids = (prefix, count) =>
  Array.from(
    { length: count },
    (_, index) => `${prefix}-${String(index + 1).padStart(2, "0")}`,
  );

const groups = {
  all: [
    ...ids("PKG", 9),
    ...ids("SHELL", 6),
    ...ids("FX", 4),
    ...ids("NOTIFY", 4),
    ...ids("START", 3),
    ...ids("FILE", 3),
    ...ids("VAULT", 5),
    ...ids("SMTP", 5),
    ...ids("DB", 8),
    ...ids("CORE", 6),
    ...ids("UPDATE", 8),
    ...ids("SEC", 4),
    ...ids("L10N", 3),
    ...ids("A11Y", 7),
    ...ids("PERF", 7),
  ],
  macOnly: ["PKG-01", "PKG-03", "FX-01", "A11Y-04"],
  windowsOnly: ["PKG-02", "FX-02", "A11Y-05"],
};

export const REQUIRED_TEST_IDS = [...new Set(groups.all)].sort();

const requiredEvidence = new Map();
function evidence(types, testIds) {
  for (const testId of testIds) {
    if (requiredEvidence.has(testId)) {
      throw new Error(`Duplicate evidence definition for ${testId}`);
    }
    requiredEvidence.set(testId, types);
  }
}

evidence(["BUILD"], ["PKG-01", "PKG-02", "PKG-03", "PKG-09", "UPDATE-07"]);
evidence(
  ["SMOKE"],
  [
    "PKG-04",
    "PKG-05",
    "PKG-06",
    "PKG-07",
    ...ids("SHELL", 6),
    "NOTIFY-01",
    "NOTIFY-02",
    "NOTIFY-03",
    ...ids("START", 3),
    "FILE-01",
    "VAULT-01",
    "VAULT-04",
    "UPDATE-04",
    "UPDATE-05",
    "UPDATE-06",
  ],
);
evidence(
  ["MANUAL"],
  [
    "PKG-08",
    "FX-01",
    "FX-02",
    "FX-03",
    "L10N-03",
    "A11Y-03",
    "A11Y-04",
    "A11Y-05",
    "A11Y-06",
  ],
);
evidence(
  ["AUTO", "SMOKE"],
  [
    "FX-04",
    "NOTIFY-04",
    "VAULT-02",
    "DB-01",
    "DB-02",
    "DB-04",
    "DB-05",
    "DB-06",
    "CORE-04",
    "CORE-06",
    "UPDATE-01",
  ],
);
evidence(["AUTO", "SECURITY"], ["FILE-02", "PERF-07"]);
evidence(["LIVE", "SECURITY"], ["FILE-03", "SEC-04"]);
evidence(["AUTO"], [
  "VAULT-03",
  "SMTP-03",
  "DB-03",
  "DB-07",
  "CORE-01",
  "CORE-02",
  "CORE-03",
  "UPDATE-02",
  "SEC-01",
  "L10N-01",
  "L10N-02",
  "A11Y-01",
  "PERF-01",
]);
evidence(["SMOKE", "SECURITY"], ["VAULT-05", "DB-08"]);
evidence(["LIVE"], ["SMTP-01", "SMTP-02", "PERF-02", "PERF-03", "PERF-05"]);
evidence(["AUTO", "LIVE"], ["SMTP-04", "CORE-05", "UPDATE-03"]);
evidence(["SECURITY"], ["SMTP-05", "UPDATE-08", "SEC-03"]);
evidence(["AUTO", "BUILD"], ["SEC-02"]);
evidence(["AUTO", "MANUAL"], ["A11Y-02", "A11Y-07"]);
evidence(["SMOKE"], ["PERF-04"]);
evidence(["LIVE", "SECURITY"], ["PERF-06"]);

if (requiredEvidence.size !== REQUIRED_TEST_IDS.length) {
  const missing = REQUIRED_TEST_IDS.filter(
    (testId) => !requiredEvidence.has(testId),
  );
  throw new Error(`Missing evidence definitions: ${missing.join(", ")}`);
}

export function testAppliesTo(testId, environmentId) {
  const isMac = environmentId.startsWith("M");
  if (groups.macOnly.includes(testId)) return isMac;
  if (groups.windowsOnly.includes(testId)) return !isMac;
  return true;
}

const coreEnvironmentIds = ["M15-A", "M15-I", "MC-A", "W25-X", "W26-X"];
const specializedRecords = [
  ["W25-X", "WEBVIEW-01", ["SMOKE"], true],
  ["W25-B", "WEBVIEW-02", ["SMOKE"], true],
  ["W25-F", "WEBVIEW-03", ["SMOKE"], true],
  ["W25-U", "WEBVIEW-04", ["SMOKE"], true],
  ["M14-C", "COMPAT-01", ["SMOKE"], false],
  ["W10-C", "COMPAT-02", ["SMOKE"], false],
  ["W24-C", "COMPAT-03", ["SMOKE"], false],
];

export const REQUIRED_RECORDS = Object.fromEntries([
  ...coreEnvironmentIds.flatMap((environmentId) =>
    REQUIRED_TEST_IDS.filter((testId) =>
      testAppliesTo(testId, environmentId),
    ).map((testId) => [
      `${environmentId}/${testId}`,
      {
        environmentId,
        testId,
        requiredEvidenceTypes: requiredEvidence.get(testId),
        blocking: true,
      },
    ]),
  ),
  ...specializedRecords.map(
    ([environmentId, testId, requiredEvidenceTypes, blocking]) => [
      `${environmentId}/${testId}`,
      { environmentId, testId, requiredEvidenceTypes, blocking },
    ],
  ),
]);

export function requiredRecordKeys() {
  return Object.keys(REQUIRED_RECORDS);
}

export function expectedPlatformIdentity(environmentId) {
  if (environmentId === "M15-A") {
    return { cpu: "arm64", osBuild: /^macOS 15\.\d+(?:\.\d+)? \(build [^)]+\)$/ };
  }
  if (environmentId === "M15-I") {
    return {
      cpu: "x86_64",
      osBuild: /^macOS 15\.\d+(?:\.\d+)? \(build [^)]+\)$/,
    };
  }
  if (environmentId === "MC-A") {
    return { cpu: "arm64", osBuild: /^macOS \d+\.\d+(?:\.\d+)? \(build [^)]+\)$/ };
  }
  if (environmentId === "M14-C") {
    return { cpu: "arm64", osBuild: /^macOS 14\.\d+(?:\.\d+)? \(build [^)]+\)$/ };
  }
  if (environmentId === "W10-C") {
    return { cpu: "x64", osBuild: /^Windows 10 22H2 \(build 19045\.\d+\)$/ };
  }
  if (environmentId.startsWith("W25")) {
    return { cpu: "x64", osBuild: /^Windows 11 25H2 \(build \d+\.\d+\)$/ };
  }
  if (environmentId === "W26-X") {
    return { cpu: "x64", osBuild: /^Windows 11 26H1 \(build \d+\.\d+\)$/ };
  }
  if (environmentId === "W24-C") {
    return { cpu: "x64", osBuild: /^Windows 11 24H2 \(build \d+\.\d+\)$/ };
  }
  throw new Error(`Unknown release environment: ${environmentId}`);
}
