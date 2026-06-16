interface AtollLogoProps {
  size?: number;
  className?: string;
}

/**
 * Neutral product mark: a minimal atoll seen from above — a thin coral ring
 * around a lagoon with a small central islet. Reads as the app itself rather
 * than any single agent's mascot.
 */
export function AtollLogo({ size = 64, className }: AtollLogoProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <defs>
        <linearGradient
          id="atoll-reef"
          x1="16"
          y1="11"
          x2="48"
          y2="53"
          gradientUnits="userSpaceOnUse"
        >
          <stop stopColor="#ffd9a4" />
          <stop offset="1" stopColor="#ff8f6e" />
        </linearGradient>
        <linearGradient
          id="atoll-lagoon"
          x1="21"
          y1="21"
          x2="43"
          y2="43"
          gradientUnits="userSpaceOnUse"
        >
          <stop stopColor="#8fe3ee" />
          <stop offset="1" stopColor="#359dc3" />
        </linearGradient>
      </defs>
      <circle cx="32" cy="32" r="22" fill="url(#atoll-reef)" />
      <circle cx="32" cy="32" r="13" fill="url(#atoll-lagoon)" />
      <circle cx="32" cy="32" r="3" fill="#fff3e2" />
    </svg>
  );
}
