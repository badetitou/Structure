import { describe, expect, it } from "vitest";
import { fit, oneToOne, panBy, resize, toUser, zoomAt, zoomAtClamped, zoomLevel } from "./viewport";

const close = (a: number, b: number) => expect(a).toBeCloseTo(b, 9);

describe("fit", () => {
  it("contains the content, matches the pane aspect and centres", () => {
    const vb = fit(400, 100, 800, 400);
    close(vb.w / vb.h, 2);
    close(vb.w, 400);
    close(vb.h, 200);
    close(vb.x, 0);
    close(vb.y, -50);
  });
  it("tall content is centred horizontally", () => {
    const vb = fit(100, 400, 800, 400);
    close(vb.h, 400);
    close(vb.w, 800);
    close(vb.x, -350);
  });
});

describe("oneToOne", () => {
  it("is pixel scale anchored with a margin", () => {
    expect(oneToOne(800, 600, 16)).toEqual({ x: -16, y: -16, w: 800, h: 600 });
  });
});

describe("toUser", () => {
  it("maps screen px to user coords", () => {
    expect(toUser({ x: 10, y: 20, w: 200, h: 100 }, 400, 100, 800, 400)).toEqual({ x: 110, y: 45 });
  });
});

describe("zoomAt", () => {
  it("keeps the point under the cursor fixed", () => {
    const vb = { x: 5, y: -7, w: 300, h: 200 };
    const [sx, sy] = [250, 90];
    const p = toUser(vb, sx, sy, 600, 400);
    for (const f of [2, 0.5, 1.37]) {
      const z = zoomAt(vb, f, p.x, p.y);
      const q = toUser(z, sx, sy, 600, 400);
      close(q.x, p.x);
      close(q.y, p.y);
      close(z.w, vb.w / f);
    }
  });
  it("ignores invalid factors", () => {
    const vb = { x: 0, y: 0, w: 1, h: 1 };
    expect(zoomAt(vb, 0, 0, 0)).toBe(vb);
    expect(zoomAt(vb, NaN, 0, 0)).toBe(vb);
  });
});

describe("zoomAtClamped", () => {
  const f = fit(1000, 500, 800, 400);
  it("clamps to 10x of fit and 0.1x of fit", () => {
    let vb = f;
    for (let i = 0; i < 50; i++) vb = zoomAtClamped(vb, 2, 500, 250, f);
    close(zoomLevel(vb, f), 10);
    for (let i = 0; i < 50; i++) vb = zoomAtClamped(vb, 0.5, 500, 250, f);
    close(zoomLevel(vb, f), 0.1);
  });
  it("keeps the anchor fixed when not clamped", () => {
    const p = toUser(f, 200, 100, 800, 400);
    const z = zoomAtClamped(f, 3, p.x, p.y, f);
    const q = toUser(z, 200, 100, 800, 400);
    close(q.x, p.x);
    close(q.y, p.y);
  });
});

describe("panBy", () => {
  it("moves content with the pointer", () => {
    const vb = panBy({ x: 0, y: 0, w: 400, h: 200 }, 100, -50, 800, 400);
    expect(vb).toEqual({ x: -50, y: 25, w: 400, h: 200 });
  });
});

describe("resize", () => {
  it("keeps scale and origin", () => {
    const vb = resize({ x: 3, y: 4, w: 400, h: 200 }, 800, 400, 100);
    expect(vb).toEqual({ x: 3, y: 4, w: 200, h: 50 });
  });
});
