import { describe, expect, test } from "bun:test";

import { isTelephoneSubscriber, withoutNumberSpaces } from "../src/tel";

// The same cases as `mailcal_composer::tel`'s tests, so the editor and the core cannot disagree.
describe("a telephone link's number", () => {
  test("accepts the numbers RFC 3966 writes", () => {
    for (const good of [
      "+15032223333",
      "+44-20-7946-0958;ext=1234",
      "+1(503)222.3333",
      "+1-201-555-0123;isub=1234",
      "+15032223333;foo=bar",
      "7042;phone-context=example.com",
      "863-1234;phone-context=+1-914-555",
      "a1b2;phone-context=example.com.",
      "7042;PHONE-CONTEXT=example.com;EXT=12",
    ]) {
      expect(isTelephoneSubscriber(good)).toBe(true);
    }
  });

  test("refuses service codes written out or encoded", () => {
    for (const code of [
      "*21*+31612345678#",
      "*61#;phone-context=example.com",
      "+1503#",
      "+1503;ext=1*2",
      "+1503;x=%2A21%23",
      "+1503;x=%2a",
      "+1503;isub=%23",
    ]) {
      expect(isTelephoneSubscriber(code)).toBe(false);
    }
  });

  test("refuses what the grammar does not write", () => {
    for (const bad of [
      "",
      "+",
      "+-()",
      "+1 503",
      "+1503;",
      "+1503;ext=",
      "+1503;x=%+1",
      "+1503;x=%2",
      "+1503;phone-context=example.com",
      "112",
      "7042;phone-context=example.com;phone-context=example.org",
      "7042;phone-context=-example.com",
      "7042;phone-context=example.1com",
      "15032223333",
    ]) {
      expect(isTelephoneSubscriber(bad)).toBe(false);
    }
  });

  test("drops the spaces of a number and nothing else", () => {
    expect(withoutNumberSpaces("tel:+1 503 222 3333")).toBe("tel:+15032223333");
    expect(withoutNumberSpaces("TEL:+1%20503 222")).toBe("TEL:+1503222");
    expect(withoutNumberSpaces("https://a b/%20")).toBe("https://a b/%20");
  });
});
