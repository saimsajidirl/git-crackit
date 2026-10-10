import React, { useState } from "react";
import { colorFor, initials } from "../util";

// GitHub noreply emails look like `12345678+login@users.noreply.github.com` —
// the numeric id maps directly to the user's avatar without an API call.
const NOREPLY = /^(\d+)\+[^@]+@users\.noreply\.github\.com$/;

export function Avatar({
  name,
  email,
  url,
  size = 28,
}: {
  name: string;
  email: string;
  url?: string | null;
  size?: number;
}) {
  const [imgFailed, setImgFailed] = useState(false);
  const m = email?.match(NOREPLY);
  const src = url ?? (m ? `https://avatars.githubusercontent.com/u/${m[1]}?s=${size * 2}` : null);

  if (src && !imgFailed) {
    return (
      <img
        className="avatar avatar-img"
        src={src}
        width={size}
        height={size}
        alt=""
        title={`${name} <${email}>`}
        loading="lazy"
        onError={() => setImgFailed(true)}
      />
    );
  }
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
