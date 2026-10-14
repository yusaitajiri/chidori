/**
 * Kikitori's logo: the name in round strokes with its mon, 丸に波に千鳥, as the o: the small drawing
 * of the Kudagitsune mon (its ring the letter's round), as in `docs/images/kikitori-logo.svg`, in the
 * theme's ink and 朱.
 */
export function Logo({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="-12 -186 793 186" className={className} role="img" aria-label="Kikitori">
      <g className="stroke-fg" fill="none" strokeWidth={24} strokeLinecap="round" strokeLinejoin="round">
        <path d="M0 -174L0 -12M0 -50L60 -108M21.6 -70.88L64 -12" />
        <path d="M124 -108L124 -12" />
        <path d="M194 -174L194 -12M194 -50L254 -108M215.6 -70.88L258 -12" />
        <path d="M318 -108L318 -12" />
        <path d="M396 -146L396 -42A30 30 0 0 0 426 -12L432 -12M368 -108L438 -108" />
        <path d="M677.47 -108L677.47 -12M677.47 -48C677.47 -90 693.47 -108 723.47 -108" />
        <path d="M765.47 -108L765.47 -12" />
      </g>
      <g className="fill-fg">
        <circle cx="124" cy="-160.88" r="14.88" />
        <circle cx="318" cy="-160.88" r="14.88" />
        <circle cx="765.47" cy="-160.88" r="14.88" />
      </g>
      <g transform="translate(498.1 -120.9) scale(0.87)">
        <circle cx="70" cy="70" r="62" fill="none" className="stroke-rec" strokeWidth={14} />
        <g transform="translate(70 70) scale(0.66) translate(-70 -70)">
          <g transform="translate(66 52) scale(0.5)" className="fill-rec">
            <path d="M-83 -36.9L-11.84 -52.71A54 54 0 1 1 -47.14 26.57Z" />
            <path d="M47.94 0.82L78.47 -19.6L41.93 -23.29Z" />
          </g>
          <path
            d="M16 104C28 90 42 90 52 104C62 118 76 118 88 104C98 92 112 92 124 104"
            fill="none"
            className="stroke-rec"
            strokeWidth={16}
            strokeLinecap="round"
          />
        </g>
      </g>
    </svg>
  );
}
