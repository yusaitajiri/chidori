// The look of a Chidori transcript (export/typst_pdf.rs). The session follows this preamble as
// calls to `header`, `entry`, `shot`, `note` and `cut`, its text in strings, so none of it is read as
// markup. The kamon brand's light colours (tokens/tokens.json in @yusai/kamon-react) on the
// paper itself: ink, and Chidori's 朱 for the voice you hear (相手).

#let ink = rgb("#2A2826")
#let muted = rgb("#5E5A53")
#let shu = rgb("#B0501F")
#let hairline = rgb("#DCD8CF")
#let rule = rgb("#C2BDB2")

#set page(
  paper: "a4",
  margin: (x: 18mm, top: 18mm, bottom: 20mm),
  // The app's name, small; the page number on the right.
  footer: context {
    set text(6.5pt, fill: muted)
    grid(
      columns: (1fr, 1fr),
      align: (left + horizon, right + horizon),
      [Chidori],
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
  // 子持ち罫, the kamon's double line that marks the top of a printed page: thick over thin.
  stack(spacing: 1.2pt, line(length: 100%, stroke: 1.2pt + rule), line(length: 100%, stroke: 0.6pt + rule))
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
