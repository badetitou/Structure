/** Pure commit-graph lane layout (newest commit first). */

export interface LaneCommit {
  id: string;
  parents: string[];
}

/** A line between lane `from` and lane `to`; see `LaneRow` for where it starts and ends. */
export interface Segment {
  from: number;
  to: number;
  color: string;
}

export interface LaneRow {
  /** Lane of this commit's dot. */
  lane: number;
  color: string;
  /** Lines from the row top (lane `from`) down to the dot (`to` = `lane`): own lane and lanes merging in. */
  top: Segment[];
  /** Lanes passing straight through the whole row. */
  through: Segment[];
  /** Lines from the dot (`from` = `lane`) to the row bottom (`to` = parent lane). */
  bottom: Segment[];
}

export interface LaneLayout {
  rows: LaneRow[];
  /** Max number of lanes used by any row. */
  width: number;
}

export const PALETTE = [
  "var(--lane-0)",
  "var(--lane-1)",
  "var(--lane-2)",
  "var(--lane-3)",
  "var(--lane-4)",
  "var(--lane-5)",
  "var(--lane-6)",
  "var(--lane-7)",
];

export function layoutLanes(commits: LaneCommit[]): LaneLayout {
  const inList = new Set(commits.map((c) => c.id));
  const active: (string | null)[] = [];
  const colors: string[] = [];
  let next = 0;
  let width = 0;
  const alloc = (id: string): number => {
    let i = active.indexOf(null);
    if (i < 0) i = active.length;
    active[i] = id;
    colors[i] = PALETTE[next++ % PALETTE.length];
    return i;
  };

  const rows: LaneRow[] = commits.map((c) => {
    const before = active.slice();
    let lane = active.indexOf(c.id);
    if (lane < 0) lane = alloc(c.id);
    const color = colors[lane];
    const top: Segment[] = [];
    const through: Segment[] = [];
    const bottom: Segment[] = [];
    width = Math.max(width, active.length);

    for (let i = 0; i < before.length; i++) {
      if (before[i] === c.id) {
        top.push({ from: i, to: lane, color: colors[i] });
        if (i !== lane) active[i] = null;
      } else if (before[i] !== null && before[i] !== undefined) {
        through.push({ from: i, to: i, color: colors[i] });
      }
    }

    const [first, ...rest] = c.parents;
    if (first === undefined) {
      active[lane] = null;
    } else {
      bottom.push({ from: lane, to: lane, color });
      active[lane] = inList.has(first) ? first : null;
    }
    for (const p of rest) {
      if (!inList.has(p)) continue;
      let t = active.indexOf(p);
      if (t < 0) t = alloc(p);
      bottom.push({ from: lane, to: t, color: colors[t] });
    }
    width = Math.max(width, active.length);
    while (active.length > 0 && active[active.length - 1] === null) active.pop();
    return { lane, color, top, through, bottom };
  });
  return { rows, width };
}
