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

type SiegePhase = "distant" | "approaching" | "assault" | "breach" | "relief";

export type SiegeBattleState = Readonly<{
  advance: number;
  phase: SiegePhase;
  threat: number;
}>;

export function createSiegeBattleState(
  weeklyRemaining: number,
  pressure: QuotaPressure,
): SiegeBattleState {
  const supply = Math.min(100, Math.max(0, weeklyRemaining));
  const threat = 100 - supply;
  const phase: Record<QuotaPressure, SiegePhase> = {
    safe: "distant",
    warning: "approaching",
    danger: "assault",
    critical: "breach",
    recovery: "relief",
  };
  const quotaAdvance = 10 + Math.pow(threat / 100, 1.18) * 68;

  return {
    advance: pressure === "recovery"
      ? 8
      : Number(quotaAdvance.toFixed(2)),
    phase: phase[pressure],
    threat,
  };
}

function siegePhaseLabel(phase: SiegePhase, locale: InterfaceLocale): string {
  const labels: Record<SiegePhase, readonly [string, string]> = {
    distant: ["远距侦测", "DISTANT"],
    approaching: ["接近防区", "APPROACHING"],
    assault: ["正在围攻", "UNDER SIEGE"],
    breach: ["防线破口", "BREACH"],
    relief: ["尸群撤退", "RETREATING"],
  };
  const [zh, en] = labels[phase];
  return locale === "zh-CN" ? zh : en;
}

export function LastSupplyLineScene({
  snapshot,
  displayMode,
  motionActive,
}: StorySceneProps) {
  const { locale, text } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const valueRef = useRef<HTMLElement>(null);
  const previousSupplyRef = useRef(snapshot.weeklyRemaining);
  const pointerFrameRef = useRef<number | null>(null);
  const pendingPointerRef = useRef<readonly [number, number] | null>(null);
  const state = siegeState(snapshot.pressure, locale);
  const supply = snapshot.weeklyRemaining;
  const battle = createSiegeBattleState(supply, snapshot.pressure);
  const combatState = snapshot.pressure === "safe"
    ? "idle"
    : snapshot.pressure === "recovery"
      ? "relief"
      : "active";
  const phaseLabel = siegePhaseLabel(battle.phase, locale);
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
    if (previousSupply === supply || !motionActive) {
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
  }, [motionActive, supply]);

  const applyParallax = (pointerX: number, pointerY: number) => {
    rootRef.current?.style.setProperty("--story-pointer-x", pointerX.toFixed(3));
    rootRef.current?.style.setProperty("--story-pointer-y", pointerY.toFixed(3));
  };
  const resetParallax = () => {
    if (pointerFrameRef.current !== null) {
      window.cancelAnimationFrame(pointerFrameRef.current);
      pointerFrameRef.current = null;
    }
    pendingPointerRef.current = null;
    rootRef.current?.style.setProperty("--story-pointer-x", "0");
    rootRef.current?.style.setProperty("--story-pointer-y", "0");
  };

  useEffect(() => {
    if (!motionActive) resetParallax();
    return () => {
      if (pointerFrameRef.current !== null) {
        window.cancelAnimationFrame(pointerFrameRef.current);
      }
    };
  }, [motionActive]);

  return (
    <div
      ref={rootRef}
      class={`primary-stat supply-line pressure-${snapshot.pressure} supply-${supplyBand}`}
      data-story-theme="last_supply_line"
      data-story-display={displayMode}
      data-story-motion={motionActive ? "active" : "paused"}
      data-siege-phase={battle.phase}
      data-siege-combat={combatState}
      role="group"
      aria-label={text(
        `七日围城：周补给剩余 ${snapshot.weeklyRemainingLabel}，${state}，${phaseLabel}。消耗速度 ${pace}。补给信号 ${signal}。`,
        `Last Supply Line: ${snapshot.weeklyRemainingLabel} weekly supplies remain. ${state}, ${phaseLabel}. Burn rate ${pace}. Supply signal ${signal}.`,
      )}
      style={`--siege-advance:${String(battle.advance)}%;--supply-level:${String(supply)}%;--threat-level:${String(battle.threat)}%;--story-pointer-x:0;--story-pointer-y:0`}
      onPointerMove={(event) => {
        if (!motionActive) return;
        const bounds = event.currentTarget.getBoundingClientRect();
        if (bounds.width === 0 || bounds.height === 0) {
          return;
        }
        const pointerX = ((event.clientX - bounds.left) / bounds.width - 0.5) * 2;
        const pointerY = ((event.clientY - bounds.top) / bounds.height - 0.5) * 2;
        pendingPointerRef.current = [pointerX, pointerY];
        if (pointerFrameRef.current !== null) return;
        pointerFrameRef.current = window.requestAnimationFrame(() => {
          pointerFrameRef.current = null;
          const pending = pendingPointerRef.current;
          pendingPointerRef.current = null;
          if (pending !== null) applyParallax(pending[0], pending[1]);
        });
      }}
      onPointerLeave={resetParallax}
      onBlur={resetParallax}
    >
      <div class="supply-line__scene" aria-hidden="true">
        <span class="supply-line__atmosphere supply-line__atmosphere--far" />
        <span class="supply-line__moon" />
        <span class="supply-line__skyline" />
        <span class="supply-line__alarm" />
        <span class="supply-line__spotlight" />
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
      <div class="supply-line__threat">
        <span>{text("战线", "THREAT")}</span>
        <strong>{phaseLabel}</strong>
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
