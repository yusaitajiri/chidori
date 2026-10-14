// The look of a Kikitori transcript (export/typst_pdf.rs). The session follows this preamble as
// calls to `header`, `entry`, `shot`, `note` and `cut`, its text in strings, so none of it is read as
// markup. White paper, ink, and 朱 (vermilion) for the voice you hear (相手).

#let ink = rgb("#121214")
#let muted = rgb("#66666d")
#let shu = rgb("#b0501f")
#let hairline = rgb("#e3e3e6")

// Kikitori's mon, 丸に波に千鳥, in its small drawing (Kudagitsune brand, mon/kikitori-small.svg).
#let mon(height) = image(
  bytes(
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 140 140'><g fill='none' stroke='#b0501f'>"
      + "<circle cx='70' cy='70' r='62' stroke-width='14'/>"
      + "<g transform='translate(70 70) scale(0.66) translate(-70 -70)'>"
      + "<g transform='translate(66 52) scale(0.5)' fill='#b0501f' stroke='none'>"
      + "<path d='M-83 -36.9L-11.84 -52.71A54 54 0 1 1 -47.14 26.57Z'/>"
      + "<path d='M47.94 0.82L78.47 -19.6L41.93 -23.29Z'/></g>"
      + "<path d='M16 104C28 90 42 90 52 104C62 118 76 118 88 104C98 92 112 92 124 104'"
      + " stroke-width='16' stroke-linecap='round'/></g></g></svg>",
  ),
  format: "svg",
  height: height,
)

#set page(
  paper: "a4",
  margin: (x: 18mm, top: 18mm, bottom: 20mm),
  // The app's name, small, with its mon; the page number on the right.
  footer: context {
    set text(6.5pt, fill: muted)
    grid(
      columns: (1fr, 1fr),
      align: (left + horizon, right + horizon),
      [#box(baseline: 22%, mon(6.5pt))#h(1.2pt) Kikitori],
      counter(page).display("1 / 1", both: true),
    )
  },
)
#set text(font: ("Yu Gothic", "Meiryo", "MS Gothic"), size: 10.5pt, lang: "ja", fill: ink)
#set par(leading: 0.85em, spacing: 0.95em)

// The title, then the date, length and sources.
#let header(title, meta) = {
  text(17pt, weight: "bold", title)
  v(-0.3em)
  text(9pt, fill: muted, meta)
  v(0.2em)
  line(length: 100%, stroke: 0.6pt + ink)
  v(0.4em)
}

// One paragraph: its time and who spoke (both optional), then what was said. A line the user
// marked important starts with a star and is set in bold.
#let entry(time: none, label: none, me: false, important: false, body) = par({
  if important { text(fill: ink, "★") + h(0.4em) }
  if time != none { text(fill: muted, number-width: "tabular", "[" + time + "]") + h(0.4em) }
  if label != none { text(weight: "bold", fill: if me { ink } else { shu }, label) + h(0.4em) }
  if important { text(weight: "bold", body) } else { body }
})

// A screenshot, its width chosen so a tall one stays on the page, with its caption.
#let shot(path, width, caption) = block(breakable: false, above: 1em, below: 1.1em, {
  box(stroke: 0.5pt + hairline, image(path, width: width))
  v(-0.4em)
  text(9pt, fill: muted, caption)
})

// A pause, a reconnected source or audio left unprocessed.
#let note(body) = align(center, text(9.5pt, fill: muted, style: "italic", body))

// A cut: a new part of the session, its time over a hairline. A heading, so the PDF's outline
// lists the parts.
#show heading.where(level: 2): it => block(above: 1.6em, below: 0.9em, sticky: true, {
  set text(9.5pt, weight: "bold", fill: muted, number-width: "tabular")
  it.body
  v(-0.55em)
  line(length: 100%, stroke: 0.5pt + hairline)
})
#let cut(time) = heading(level: 2, time)
