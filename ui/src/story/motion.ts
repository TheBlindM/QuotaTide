import { useEffect, useState } from "preact/hooks";

function motionMediaQuery(): MediaQueryList | null {
  if (typeof window.matchMedia !== "function") return null;
  const matchMedia = window.matchMedia.bind(window) as (
    query: string,
  ) => MediaQueryList | undefined;
  return matchMedia("(prefers-reduced-motion: reduce)") ?? null;
}

function reducedMotionRequested(): boolean {
  return motionMediaQuery()?.matches ?? false;
}

export function useStoryMotionActive(): boolean {
  const [windowFocused, setWindowFocused] = useState(true);
  const [motionActive, setMotionActive] = useState(
    () => document.visibilityState !== "hidden" && !reducedMotionRequested(),
  );

  useEffect(() => {
    const media = motionMediaQuery();
    const sync = (focused = windowFocused) => {
      setMotionActive(
        focused && document.visibilityState !== "hidden" && !(media?.matches ?? false),
      );
    };
    const onBlur = () => {
      setWindowFocused(false);
      sync(false);
    };
    const onFocus = () => {
      setWindowFocused(true);
      sync(true);
    };
    const onVisibilityChange = () => {
      sync(windowFocused);
    };

    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);
    media?.addEventListener("change", onVisibilityChange);
    sync();
    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      media?.removeEventListener("change", onVisibilityChange);
    };
  }, [windowFocused]);

  return motionActive;
}
