import { describe, expect, it } from "vitest";
import { prTabName, selectionClasses } from "./tab";

const PR = "/o/r/pull/7";

describe("prTabName", () => {
  it("names the PR's own tabs, absolute or relative", () => {
    expect(prTabName(PR, PR)).toBe("overview");
    expect(prTabName(`${PR}/commits`, PR)).toBe("commits");
    expect(prTabName(`https://github.com${PR}/checks`, PR)).toBe("checks");
    expect(prTabName(`${PR}/files?w=1`, PR)).toBe("files");
    expect(prTabName(`${PR}/changes#diff`, PR)).toBe("changes");
  });
  it("ignores other links", () => {
    expect(prTabName("/o/r/pull/70/commits", PR)).toBeNull();
    expect(prTabName(`${PR}/commits/abc123`, PR)).toBeNull();
    expect(prTabName("#structure", PR)).toBeNull();
    expect(prTabName("/o/r/issues", PR)).toBeNull();
  });
});

describe("selectionClasses", () => {
  it("keeps plain and hashed selected classes only", () => {
    expect(selectionClasses("TabNavLink__x selected__NLIm71 flex-shrink-0")).toEqual(["selected__NLIm71"]);
    expect(selectionClasses("tabnav-tab selected")).toEqual(["selected"]);
    expect(selectionClasses("prc-UnderlineNav-UnderlineItem")).toEqual([]);
  });
});
