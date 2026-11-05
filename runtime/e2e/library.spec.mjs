import { test, expect } from "@playwright/test";
for (const backend of ["core", "component"])
  test(`${backend}: browser text library preserves controls and 64-bit integers`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("text-library/index.js", location.href).href
      );
      let memory;
      const { api } = await instantiate({
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) memory = instance.exports;
          return instance;
        },
      });
      const text = '"\\\u0000\n日本語🦀';
      return {
        roundtrip: JSON.parse(api.quote(text)) === text,
        joined: api.join("日本語", "🦀"),
        unsigned: api.unsigned(-1n),
        signed: api.signed(-9223372036854775808n),
        split: api.split("🦀::日本語::", "::"),
        parsed: BigInt.asUintN(64, api.parseUnsigned("18446744073709551615")).toString(),
        overflow: api.parseSigned("-9223372036854775809") === undefined,
        inside: api.findText("🦀", "", 1) === undefined,
        repeated: api.repeat("🦀", 2048).length,
        document: JSON.parse(api.document('key"', text, 42n)),
        live: memory.live(),
      };
    });
    expect(result).toEqual({
      roundtrip: true,
      joined: "日本語🦀",
      unsigned: "18446744073709551615",
      signed: "-9223372036854775808",
      split: ["🦀", "日本語::"],
      parsed: "18446744073709551615",
      overflow: true,
      inside: true,
      repeated: 4096,
      document: { ['key"']: '"\\\u0000\n日本語🦀', unsigned: 42, signed: 42, ok: true },
      live: 0,
    });
  });
