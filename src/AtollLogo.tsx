interface AtollLogoProps {
  size?: number;
  className?: string;
}

/**
 * Neutral product mark: a minimal atoll seen from above — a bold gradient
 * reef ring around a bright central islet.  Two elements only so the mark
 * stays crisp at small sizes (≤ 16 px).
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
          id="atoll-ring-g"
          x1="8"
          y1="8"
          x2="56"
          y2="56"
          gradientUnits="userSpaceOnUse"
        >
          <stop stopColor="#5EEAD4" />
          <stop offset="0.5" stopColor="#38BDF8" />
          <stop offset="1" stopColor="#818CF8" />
        </linearGradient>
      </defs>
      <circle
        cx="32"
        cy="32"
        r="22"
        stroke="url(#atoll-ring-g)"
        strokeWidth="8"
        strokeLinecap="round"
      />
      <circle cx="32" cy="32" r="4.5" fill="white" fillOpacity="0.92" />
    </svg>
  );
}
