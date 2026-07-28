function normalizeWatch(watch, now) {
  const chancePercent = Number(watch?.reset_chance_24h);
  const expiresAtMs = Date.parse(watch?.expires_at || "");
  if (
    !Number.isFinite(chancePercent) ||
    chancePercent < 0 ||
    chancePercent > 100 ||
    !Number.isFinite(expiresAtMs) ||
    expiresAtMs <= now
  ) {
    return null;
  }

  return {
    level: String(watch.level || ""),
    chancePercent,
    observedAt: String(watch.observed_at || ""),
    expiresAt: String(watch.expires_at),
    windowHours: Number(watch.window_hours) || 24,
    source: {
      id: String(watch.tweet_id || ""),
      url: String(watch.tweet_url || ""),
      text: String(watch.text || ""),
    },
  };
}

export function normalizeRadar(payload, now = Date.now()) {
  const events = Array.isArray(payload?.events) ? payload.events : [];
  const latest = events[0];
  return {
    generatedAt: payload?.generated_at || null,
    total: Number(payload?.stats?.total) || events.length,
    averageIntervalDays: Number(payload?.stats?.avg_interval_days) || null,
    watch: normalizeWatch(payload?.watch, now),
    latest: latest
      ? {
          id: String(latest.tweet_id || ""),
          url: String(latest.tweet_url || ""),
          text: String(latest.text || ""),
          announcedAt: String(latest.announced_at || ""),
        }
      : null,
  };
}

export async function fetchResetRadar(config, fetchImpl = fetch) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  try {
    const response = await fetchImpl(config.codexResetsUrl, {
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`重置雷达返回 HTTP ${response.status}`);
    }
    return normalizeRadar(await response.json());
  } finally {
    clearTimeout(timeout);
  }
}
