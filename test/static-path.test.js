import test from "node:test";
import assert from "node:assert/strict";
import { decodeRequestPath } from "../src/static-path.js";

test("畸形 URL 编码返回无效结果而不是抛出异常", () => {
  assert.deepEqual(decodeRequestPath("/%zz"), { ok: false, value: "" });
});

test("有效 URL 编码正常解码", () => {
  assert.deepEqual(decodeRequestPath("/hello%20world"), {
    ok: true,
    value: "/hello world",
  });
});
