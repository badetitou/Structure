import { describe, expect, it } from "vitest";
import { symbolName } from "./symbolname";

describe("symbolName", () => {
  it("takes the last segment of Java and TS ids", () => {
    expect(symbolName("org.mybatis.jpetstore.web.AccountController")).toBe("AccountController");
    expect(symbolName("src/vs/base/a.contribution.ts#Widget")).toBe("Widget");
    expect(symbolName("Plain")).toBe("Plain");
    expect(symbolName("trailing.")).toBe("trailing.");
  });
});
