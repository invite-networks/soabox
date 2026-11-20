import { useId } from "react";

/** The SOAbox mark: a zone drawn as a short list of records. Decorative; pair it with the product name. */
export function LogoMark({ size = 28 }: { size?: number }) {
  const id = useId();
  return (
    <svg aria-hidden width={size} height={size} viewBox="0 0 32 32" className="shrink-0">
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#3b82f6" />
          <stop offset="1" stopColor="#1d4ed8" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="7" fill={`url(#${id})`} />
      <g fill="#fff">
        <circle cx="10" cy="11" r="1.75" />
        <circle cx="10" cy="16" r="1.75" />
        <circle cx="10" cy="21" r="1.75" />
      </g>
      <g stroke="#fff" strokeWidth="2.5" strokeLinecap="round">
        <path d="M14.5 11h8" />
        <path d="M14.5 16h8" opacity=".8" />
        <path d="M14.5 21h4.5" opacity=".6" />
      </g>
    </svg>
  );
}

/** The mark with the product name. */
export function Logo({ size = 28 }: { size?: number }) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <LogoMark size={size} />
      <span className="text-[15px] font-semibold tracking-tight">SOAbox</span>
    </span>
  );
}
