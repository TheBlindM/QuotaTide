export function decodeRequestPath(pathname) {
  try {
    return { ok: true, value: decodeURIComponent(pathname) };
  } catch {
    return { ok: false, value: "" };
  }
}
