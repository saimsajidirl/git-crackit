export function timeAgo(ts: number): string {
  const sec = Math.max(0, Date.now() / 1000 - ts);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} minute${min === 1 ? "" : "s"} ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} hour${hr === 1 ? "" : "s"} ago`;
  const d = Math.floor(hr / 24);
  if (d < 30) return `${d} day${d === 1 ? "" : "s"} ago`;
  const mo = Math.floor(d / 30);
  if (mo < 12) return `${mo} month${mo === 1 ? "" : "s"} ago`;
  const y = Math.floor(mo / 12);
  return `${y} year${y === 1 ? "" : "s"} ago`;
}

export function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

const PALETTE = [
  "#0969da", "#8250df", "#bf3989", "#cf222e", "#bc4c00",
  "#1a7f37", "#0a7ea4", "#6e7781", "#9a6700", "#57606a",
];

export function colorFor(str: string): string {
  let h = 0;
  for (let i = 0; i < str.length; i++) h = (h * 31 + str.charCodeAt(i)) >>> 0;
  return PALETTE[h % PALETTE.length];
}

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/);
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

export function fileName(path: string): string {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(i + 1) : path;
}

export function dirName(path: string): string {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(0, i) : "";
}

export function statusLetter(status: string | null): string {
  switch (status) {
    case "added": return "A";
    case "deleted": return "D";
    case "renamed": return "R";
    case "modified": return "M";
    case "typechange": return "T";
    case "copied": return "C";
    case "conflicted": return "!";
    default: return "M";
  }
}

export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}
