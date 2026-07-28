const $ = (id) => document.getElementById(id);

const labels = {
  normal: "正常",
  warning: "预警",
  exceeded: "超额",
  daily_warning: "今日额度预警",
  daily_exceeded: "今日额度超额",
  fetch_failed: "额度采集失败",
  reset_confirmed: "全局重置已确认",
};

function formatNumber(value, digits = 1) {
  return Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : "—";
}

function formatDateTime(timestamp, timezone) {
  if (!timestamp) return "—";
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: timezone,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(timestamp));
}

function relativeTime(timestamp) {
  if (!timestamp) return "—";
  const seconds = Math.round((new Date(timestamp).getTime() - Date.now()) / 1000);
  const formatter = new Intl.RelativeTimeFormat("zh-CN", { numeric: "auto" });
  if (Math.abs(seconds) < 3600) return formatter.format(Math.round(seconds / 60), "minute");
  if (Math.abs(seconds) < 86400) return formatter.format(Math.round(seconds / 3600), "hour");
  return formatter.format(Math.round(seconds / 86400), "day");
}

function safeExternalUrl(value, fallback = "https://codex-resets.com/") {
  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.href : fallback;
  } catch {
    return fallback;
  }
}

function renderHistory(history) {
  const chart = $("historyChart");
  if (!history?.length) {
    chart.innerHTML = '<p class="empty-state">采集后显示每日使用记录</p>';
    return;
  }
  const ordered = [...history].reverse();
  const max = Math.max(...ordered.map((day) => Math.max(day.used, day.limit)), 16);
  chart.innerHTML = ordered
    .map((day) => {
      const height = Math.min(100, (day.used / max) * 100);
      const limitHeight = Math.min(100, (day.limit / max) * 100);
      return `
        <div class="history-day" title="${day.date}：${formatNumber(day.used)}% / ${day.limit}%">
          <span class="history-value">${formatNumber(day.used)}%</span>
          <div class="history-bars">
            <span class="history-limit" style="bottom:${limitHeight}%"></span>
            <span class="history-used ${day.status}" style="height:${height}%"></span>
          </div>
          <span class="history-label">${day.date.slice(5)}</span>
        </div>`;
    })
    .join("");
}

function renderAlerts(alerts, timezone) {
  const list = $("alertList");
  if (!alerts?.length) {
    list.innerHTML = '<p class="empty-state">暂无告警</p>';
    return;
  }
  list.innerHTML = alerts
    .map(
      (alert) => `
        <div class="alert-item">
          <strong>${labels[alert.type] || alert.type}</strong>
          <span>${formatDateTime(alert.createdAt, timezone)} · ${
            alert.deliveryStatus === "sent"
              ? "已发送"
              : alert.deliveryStatus === "not_configured"
                ? "邮件未配置"
                : "发送失败"
          }</span>
        </div>`,
    )
    .join("");
}

function render(data) {
  const notice = $("notice");
  if (!data.configured) {
    notice.hidden = false;
    notice.textContent = "尚未配置 AUTH_JSON_PATH。服务已启动，配置凭证路径后即可采集真实额度。";
  } else if (data.lastFailure && !data.lastSuccessAt) {
    notice.hidden = false;
    notice.textContent = data.lastFailure.message;
  } else if (data.stale) {
    notice.hidden = false;
    notice.textContent = "额度数据已过期，当前展示最后一次成功快照。";
  } else {
    notice.hidden = true;
  }

  $("accountPlan").textContent = data.account.planType
    ? `${data.account.planType.toUpperCase()} 账号`
    : "待配置";
  $("accountEmail").textContent = data.account.email || "—";
  $("accountId").textContent = data.account.accountId || "—";
  $("pollInterval").textContent = `每 ${data.pollIntervalMinutes} 分钟`;
  $("mailStatus").textContent = data.mailEnabled ? "邮件已启用" : "邮件未配置";
  $("failureStatus").textContent = data.consecutiveFailures
    ? `连续失败 ${data.consecutiveFailures} 次`
    : "采集正常";

  const state = !data.configured
    ? "neutral"
    : data.stale || !data.quota
      ? "neutral"
      : data.today.status;
  const badge = $("statusBadge");
  badge.className = `status-badge ${state}`;
  badge.textContent =
    state === "neutral"
      ? data.configured
        ? "数据异常"
        : "未连接"
      : labels[state];

  $("todayUsed").textContent = formatNumber(data.today.used);
  $("todayLimit").textContent = `${data.today.limit}%`;
  $("limitAdjustment").textContent =
    data.today.policyKind === "weekend_fixed"
      ? "周末固定额度"
      : data.today.adjustment > 0.01
        ? `基础 ${formatNumber(data.today.baseLimit)}% + 结转 ${formatNumber(data.today.adjustment)}%`
        : "工作日基础额度";
  $("todayProgress").style.width = `${data.today.progressPercent}%`;
  $("todayProgress").className = `progress-fill ${data.today.status}`;
  $("todayState").textContent = labels[data.today.status];
  $("todayRemaining").textContent =
    data.today.used >= data.today.limit
      ? `已超出 ${formatNumber(data.today.used - data.today.limit)}%`
      : `距建议上限还有 ${formatNumber(data.today.limit - data.today.used)}%`;
  document
    .querySelector(".progress-track")
    .setAttribute("aria-valuenow", String(Math.round(data.today.progressPercent)));

  const quota = data.quota;
  $("weeklyUsed").textContent = formatNumber(quota?.usedPercent);
  $("weeklyRemaining").textContent = quota
    ? `${formatNumber(quota.remainingPercent)}%`
    : "—";
  $("weeklyProgress").style.width = quota
    ? `${Math.min(100, quota.usedPercent)}%`
    : "0";
  $("weeklyReset").textContent = quota
    ? formatDateTime(quota.resetAt * 1000, data.timezone)
    : "—";
  $("weeklyCountdown").textContent = quota
    ? relativeTime(quota.resetAt * 1000)
    : "—";
  $("resetCredits").textContent = quota ? String(quota.resetCredits) : "—";

  $("lastUpdated").textContent = data.lastSuccessAt
    ? `更新于 ${formatDateTime(data.lastSuccessAt, data.timezone)}`
    : "尚未采集";

  const radar = data.radar;
  const prediction =
    radar?.watch && Date.parse(radar.watch.expiresAt) > Date.now()
      ? radar.watch
      : null;
  const predictionWatch = $("predictionWatch");
  if (prediction) {
    predictionWatch.hidden = false;
    predictionWatch.dataset.level =
      prediction.chancePercent >= 70
        ? "high"
        : prediction.chancePercent >= 40
          ? "medium"
          : "low";
    $("predictionChance").textContent =
      prediction.displayChance || "有信号";
    $("predictionWindow").textContent = `未来 ${prediction.windowHours} 小时`;
    $("predictionMeta").textContent =
      `${relativeTime(prediction.observedAt)}发现 · ` +
      `有效至 ${formatDateTime(prediction.expiresAt, data.timezone)}`;
    $("predictionText").textContent =
      prediction.source.text || "Codex Resets 当前未提供预测说明。";
    $("predictionLink").href = safeExternalUrl(prediction.source.url);
  } else {
    predictionWatch.hidden = true;
    delete predictionWatch.dataset.level;
  }

  if (radar?.latest) {
    $("radarStatus").textContent = `共 ${radar.total} 次`;
    $("radarStatus").className = "live-dot active";
    $("radarTime").textContent = relativeTime(radar.latest.announcedAt);
    $("radarText").textContent = radar.latest.text;
    $("radarLink").href = safeExternalUrl(radar.latest.url);
    $("radarLink").textContent = "查看原公告";
  }

  renderHistory(data.history);
  renderAlerts(data.alerts, data.timezone);
}

async function loadStatus() {
  const response = await fetch("/api/status", { cache: "no-store" });
  if (!response.ok) throw new Error("无法读取监控状态");
  render(await response.json());
}

$("refreshButton").addEventListener("click", async () => {
  const button = $("refreshButton");
  button.disabled = true;
  button.textContent = "刷新中…";
  try {
    const response = await fetch("/api/refresh", { method: "POST" });
    const result = await response.json();
    if (!response.ok && result.error) throw new Error(result.error);
    await loadStatus();
  } catch (error) {
    const notice = $("notice");
    notice.hidden = false;
    notice.textContent = error.message;
  } finally {
    button.disabled = false;
    button.textContent = "刷新额度";
  }
});

loadStatus().catch((error) => {
  const notice = $("notice");
  notice.hidden = false;
  notice.textContent = error.message;
});

setInterval(loadStatus, 60_000);
