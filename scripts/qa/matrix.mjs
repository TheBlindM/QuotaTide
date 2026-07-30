export const REQUIRED_ENVIRONMENTS = {
  "M15-A": "macOS 15 latest patch / Apple Silicon",
  "M15-I": "macOS 15 latest patch / Intel",
  "MC-A": "Current stable macOS / Apple Silicon",
  "W25-X": "Windows 11 25H2 latest patch / x64",
  "W26-X": "Windows 11 26H1 latest patch / x64",
};

const groups = {
  all: [
    ...Array.from({ length: 9 }, (_, index) => `PKG-0${index + 1}`),
    ...Array.from({ length: 6 }, (_, index) => `SHELL-0${index + 1}`),
    ...Array.from({ length: 4 }, (_, index) => `FX-0${index + 1}`),
    ...Array.from({ length: 4 }, (_, index) => `NOTIFY-0${index + 1}`),
    ...Array.from({ length: 3 }, (_, index) => `START-0${index + 1}`),
    ...Array.from({ length: 3 }, (_, index) => `FILE-0${index + 1}`),
    ...Array.from({ length: 5 }, (_, index) => `VAULT-0${index + 1}`),
    ...Array.from({ length: 5 }, (_, index) => `SMTP-0${index + 1}`),
    ...Array.from({ length: 8 }, (_, index) => `DB-0${index + 1}`),
    ...Array.from({ length: 6 }, (_, index) => `CORE-0${index + 1}`),
    ...Array.from({ length: 8 }, (_, index) => `UPDATE-0${index + 1}`),
    ...Array.from({ length: 4 }, (_, index) => `SEC-0${index + 1}`),
    ...Array.from({ length: 3 }, (_, index) => `L10N-0${index + 1}`),
    ...Array.from({ length: 7 }, (_, index) => `A11Y-0${index + 1}`),
    ...Array.from({ length: 7 }, (_, index) => `PERF-0${index + 1}`),
  ],
  macOnly: ["PKG-01", "PKG-03", "FX-01", "A11Y-04"],
  windowsOnly: ["PKG-02", "FX-02", "A11Y-05"],
};

export const REQUIRED_TEST_IDS = [...new Set(groups.all)].sort();

export function testAppliesTo(testId, environmentId) {
  const isMac = environmentId.startsWith("M");
  if (groups.macOnly.includes(testId)) return isMac;
  if (groups.windowsOnly.includes(testId)) return !isMac;
  return true;
}

export function requiredRecordKeys() {
  return Object.keys(REQUIRED_ENVIRONMENTS).flatMap((environmentId) =>
    REQUIRED_TEST_IDS.filter((testId) =>
      testAppliesTo(testId, environmentId),
    ).map((testId) => `${environmentId}/${testId}`),
  );
}
