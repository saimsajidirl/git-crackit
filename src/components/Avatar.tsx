import { colorFor, initials } from "../util";

export function Avatar({ name, email, size = 28 }: { name: string; email: string; size?: number }) {
  const bg = colorFor(email || name);
  return (
    <div
      className="avatar"
      style={{ width: size, height: size, background: bg, fontSize: size * 0.4 }}
      title={`${name} <${email}>`}
    >
      {initials(name || "?")}
    </div>
  );
}
