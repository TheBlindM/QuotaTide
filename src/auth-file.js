import { readFile } from "node:fs/promises";

function decodeJwtPayload(token) {
  const parts = token.split(".");
  if (parts.length < 2) return {};
  try {
    return JSON.parse(Buffer.from(parts[1], "base64url").toString("utf8"));
  } catch {
    return {};
  }
}

function firstString(...values) {
  return values.find((value) => typeof value === "string" && value.trim())?.trim();
}

export function parseAuthJson(contents) {
  let raw;
  try {
    raw = JSON.parse(contents);
  } catch {
    throw new Error("auth.json 不是有效的 JSON");
  }

  const tokens = raw.tokens || raw.auth?.tokens || raw;
  const accessToken = firstString(
    tokens.access_token,
    tokens.accessToken,
    raw.access_token,
  );
  if (!accessToken) {
    throw new Error("auth.json 中缺少 access_token");
  }

  const accessClaims = decodeJwtPayload(accessToken);
  const idClaims = tokens.id_token
    ? decodeJwtPayload(tokens.id_token)
    : {};
  const accountId = firstString(
    tokens.account_id,
    tokens.accountId,
    raw.account_id,
    accessClaims["https://api.openai.com/auth.chatgpt_account_id"],
    idClaims["https://api.openai.com/auth.chatgpt_account_id"],
  );
  if (!accountId) {
    throw new Error("auth.json 中缺少 account_id，Token 中也无法提取");
  }

  return {
    accessToken,
    accountId,
    email: firstString(tokens.email, raw.email, idClaims.email) || "",
  };
}

export async function readAuthFile(filePath) {
  if (!filePath) {
    throw new Error("尚未配置 AUTH_JSON_PATH");
  }
  const contents = await readFile(filePath, { encoding: "utf8" });
  return parseAuthJson(contents);
}
