import { useEffect, useRef } from "preact/hooks";

import type { QuotaPressure } from "../../../bindings/QuotaPressure";
import { useI18n } from "../../../i18n-context";
import type { InterfaceLocale } from "../../../i18n";
import type { StorySceneProps } from "../../types";
import "./theme.css";

function siegeState(
  pressure: QuotaPressure,
  locale: InterfaceLocale,
): string {
  const labels: Record<QuotaPressure, readonly [string, string]> = {
    safe: ["防线稳定", "Line secure"],
    warning: ["尸群接近", "Horde approaching"],
    danger: ["防线承压", "Line under pressure"],
    critical: ["最后防线", "Last line"],
    recovery: ["补给抵达", "Supplies arrived"],
  };
  const [zh, en] = labels[pressure];
  return locale === "zh-CN" ? zh : en;
}

export function LastSupplyLineScene({ snapshot, displayMode }: StorySceneProps) {
  const { locale, text } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const valueRef = useRef<HTMLElement>(null);
  const previousSupplyRef = useRef(snapshot.weeklyRemaining);
  const state = siegeState(snapshot.pressure, locale);
  const supply = snapshot.weeklyRemaining;
  const weeklyUsed = 100 - supply;
  const advance = snapshot.pressure === "recovery"
    ? 14
    : Number((8 + weeklyUsed * 0.23).toFixed(2));
  const supplyBand = supply <= 10 ? "critical" : supply <= 25 ? "low" : "ready";
  const signalState = snapshot.pressure === "recovery"
    ? "delivered"
    : snapshot.radarChance !== null
      ? "active"
      : "scanning";
  const signal: string = signalState === "delivered"
    ? text("已抵达", "Arrived")
    : signalState === "active"
      ? snapshot.radarChance ?? "—"
      : text("搜寻中", "Scanning");
  const pace = snapshot.projection?.rate ?? text("待观测", "Observing");

  useEffect(() => {
    const previousSupply = previousSupplyRef.current;
    previousSupplyRef.current = supply;
    const reduceMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (previousSupply === supply || reduceMotion) {
      return;
    }
    const valueElement = valueRef.current;
    if (valueElement === null || typeof valueElement.animate !== "function") {
      return;
    }
    valueElement.animate(
      [
        { opacity: 0.58, transform: "translateY(4px) scale(0.96)" },
        { opacity: 1, transform: "translateY(0) scale(1)" },
      ],
      { duration: 520, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
    );
  }, [supply]);

  const resetParallax = () => {
    rootRef.current?.style.setProperty("--story-pointer-x", "0");
    rootRef.current?.style.setProperty("--story-pointer-y", "0");
  };

  return (
    <div
      ref={rootRef}
      class={`primary-stat supply-line pressure-${snapshot.pressure} supply-${supplyBand}`}
      data-story-theme="last_supply_line"
      data-story-display={displayMode}
      role="group"
      aria-label={text(
        `七日围城：周补给剩余 ${snapshot.weeklyRemainingLabel}，${state}。消耗速度 ${pace}。补给信号 ${signal}。`,
        `Last Supply Line: ${snapshot.weeklyRemainingLabel} weekly supplies remain. ${state}. Burn rate ${pace}. Supply signal ${signal}.`,
      )}
      style={`--siege-advance:${String(advance)}%;--supply-level:${String(supply)}%;--threat-level:${String(weeklyUsed)}%;--story-pointer-x:0;--story-pointer-y:0`}
      onPointerMove={(event) => {
        const bounds = event.currentTarget.getBoundingClientRect();
        if (bounds.width === 0 || bounds.height === 0) {
          return;
        }
        const pointerX = ((event.clientX - bounds.left) / bounds.width - 0.5) * 2;
        const pointerY = ((event.clientY - bounds.top) / bounds.height - 0.5) * 2;
        event.currentTarget.style.setProperty(
          "--story-pointer-x",
          pointerX.toFixed(3),
        );
        event.currentTarget.style.setProperty(
          "--story-pointer-y",
          pointerY.toFixed(3),
        );
      }}
      onPointerLeave={resetParallax}
      onBlur={resetParallax}
    >
      <div class="supply-line__scene" aria-hidden="true">
        <span class="supply-line__atmosphere supply-line__atmosphere--far" />
        <span class="supply-line__moon" />
        <span class="supply-line__skyline" />
        <span class="supply-line__scan" />
        <span class={`supply-line__radio signal-${signalState}`}>
          <i />
        </span>
        <span class="supply-line__road" />
        <span class="supply-line__threat-line" />
        <div class="supply-line__horde">
          <span class="siege-zombie siege-zombie--one" />
          <span class="siege-zombie siege-zombie--two" />
          <span class="siege-zombie siege-zombie--three" />
          <span class="siege-zombie siege-zombie--four" />
          <span class="siege-zombie siege-zombie--five" />
          <span class="siege-zombie siege-zombie--six" />
          <span class="siege-zombie siege-zombie--seven" />
          <span class="siege-zombie siege-zombie--eight" />
        </div>
        <span class="supply-line__airdrop" data-testid="supply-airdrop" />
        <div class="supply-line__defenders">
          <span class="siege-defender siege-defender--rear" />
          <span class="siege-defender siege-defender--front" />
          <span
            class="siege-defender siege-defender--rpg"
            data-testid="siege-rpg"
          />
          <span class="siege-muzzle" />
        </div>
        <span class="siege-rocket" data-testid="siege-rocket" />
        <span class="siege-blast" data-testid="siege-blast" />
        <span class="supply-line__barricade" />
        <div class="supply-line__crates">
          <span />
          <span />
          <span />
        </div>
        <span class="supply-line__atmosphere supply-line__atmosphere--near" />
      </div>
      <div class="supply-line__readout">
        <div class="supply-line__readout-copy">
          <span>{text("本周补给余量", "WEEKLY SUPPLY")}</span>
          <small>{state}</small>
        </div>
        <strong ref={valueRef}>{snapshot.weeklyRemainingLabel}</strong>
        <span class="supply-line__meter" aria-hidden="true">
          <i />
        </span>
      </div>
      <div class="supply-line__signal">
        <span class="supply-line__signal-dot" aria-hidden="true" />
        <span>{text("补给信号", "SUPPLY SIGNAL")}</span>
        <strong>{signal}</strong>
      </div>
    </div>
  );
}
