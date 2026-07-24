function asFiniteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function pickWeeklyWindow(rateLimit) {
  const windows = [
    rateLimit?.primary_window,
    rateLimit?.secondary_window,
  ].filter(Boolean);
  return (
    windows.find(
      (window) => asFiniteNumber(window.limit_window_seconds) >= 86400,
    ) || null
  );
}

export function normalizeUsage(payload) {
  const rateLimit = payload?.rate_limit;
  const weekly = pickWeeklyWindow(rateLimit);
  if (!weekly) {
    throw new Error("上游响应中没有可识别的周额度窗口");
  }

  const usedPercent = asFiniteNumber(weekly.used_percent);
  const resetAt = asFiniteNumber(weekly.reset_at);
  if (usedPercent == null || resetAt == null) {
    throw new Error("上游周额度窗口缺少 used_percent 或 reset_at");
  }

  return {
    planType: payload.plan_type || rateLimit?.plan_type || "unknown",
    userId: payload.user_id || "",
    email: payload.email || "",
    allowed: rateLimit?.allowed !== false && !rateLimit?.limit_reached,
    usedPercent: Math.max(0, usedPercent),
    remainingPercent: Math.max(0, 100 - usedPercent),
    resetAt,
    resetAfterSeconds: asFiniteNumber(weekly.reset_after_seconds),
    windowSeconds: asFiniteNumber(weekly.limit_window_seconds),
    resetCredits: Math.max(
      0,
      asFiniteNumber(payload?.rate_limit_reset_credits?.available_count) || 0,
    ),
  };
}

export async function fetchCodexUsage(config, credentials, fetchImpl = fetch) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);
  try {
    const response = await fetchImpl(
      `${config.codexBaseUrl}/backend-api/wham/usage`,
      {
        method: "GET",
        headers: {
          Authorization: `Bearer ${credentials.accessToken}`,
          "chatgpt-account-id": credentials.accountId,
          originator: "codex_cli_rs",
          Accept: "application/json",
        },
        signal: controller.signal,
      },
    );
    if (!response.ok) {
      if (response.status === 401 || response.status === 403) {
        throw new Error("Codex 凭证已失效，等待 Codex 软件刷新 Token");
      }
      throw new Error(`Codex 上游返回 HTTP ${response.status}`);
    }
    return normalizeUsage(await response.json());
  } catch (error) {
    if (error?.name === "AbortError") {
      throw new Error("Codex 上游请求超时");
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}
