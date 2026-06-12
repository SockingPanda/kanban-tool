---
version: alpha
name: GitHub
website: "https://github.com"
description: >-
  A near-black developer-platform canvas anchored on "#0d1117" (the canonical GitHub dark-mode surface) and a pale arctic-blue link voltage "#8dd6ff" used as the only text accent across hero, nav, and link rests. Display type runs Mona Sans at variable weights that land between 425 and 480 — never the bold 700 most dev platforms reach for — with aggressive negative tracking on the hero ("-2.24px" at 64px). Pill nav buttons hit a "60px" pillar radius for the segmented Code/Plan/Collaborate/Automate/Secure switcher; cards land at "16px"; CTA buttons hold "8px". Accent green "#5fed83" lights the activity-glow and CTA-banner shadow gradient; success-emphasis green "#08872b" carries the actual primary action paint. The system reads as a developer console wrapped in editorial restraint — heavy use of code-screenshots and IDE mockups framed in "#151a22" panels, with purple-to-indigo radial atmospherics ("#9350ff", "#5049c2", "#7873cb") only inside the Copilot hero glow.

seo:
  title: "GitHub Design System for React — Arctic Link Blue (#8dd6ff), Mona Sans, 19 components"
  metaDescription: "GitHub's design system as a DESIGN.md file. Canvas #0d1117, link blue #8dd6ff, accent green #5fed83, Mona Sans, 19 components. For React, Next.js, and AI tools."
  highlights:
    - "Dual-canvas system — #0d1117 for product surfaces and #000000 for hero/CTA bands, switched at section boundaries rather than blended"
    - "Arctic-blue link voltage — #8dd6ff carries every link rest, hero pill stroke, and Copilot CTA label; count 138 across the page"
    - "Two greens, two jobs — #5fed83 for activity-glow gradients and accent dots; #08872b for the success-emphasis primary CTA paint"
    - "Variable-weight Mona Sans — hero hits weight 425 at 64px with -2.24px tracking, h2 sits at weight 460, never the bold 700 most dev platforms reach for"
    - "Pill segmented control — 60px radius on the Code/Plan/Collaborate/Automate/Secure switcher; the page's dominant radius value at 12 occurrences"
  tags:
    - "Developer Tools & IDEs"
  lastUpdated: "2026-05-13"
  author:
    name: "Dov Azencot"
    url: "https://x.com/dovazencot"
  opening: |
    GitHub's marketing canvas is a developer-console aesthetic translated into magazine restraint. The page floor is "#0d1117" — the canonical dark-mode surface that ships inside the product itself — with full-bleed "#000000" bands cutting in for hero openers and the Copilot CTA region. Body and headline ink hold "#ffffff" against the dark; the single load-bearing chromatic token is an arctic pale-blue "#8dd6ff" that lights every link rest, hero pill stroke, and Copilot CTA label across the page. Where most developer platforms reach for a brand purple or electric green as primary voltage, GitHub keeps the link blue dominant and reserves "#5fed83" for activity-glow gradients only — never for buttons. The hero atmosphere is a purple-to-indigo radial ("#9350ff", "#5049c2", "#7873cb", "#0e0aa2") sitting behind the Copilot IDE mockup, a single glow event rather than a system-wide gradient language.

    This page packages the surface into one DESIGN.md file written to the Google Labs spec. Inside: 21 color tokens grouped into canvas, surface ladder, ink, link/accent, success/danger, and the indigo atmospherics; 12 typography levels covering Mona Sans variable-weight display from 64px down to 12px caption plus ui-monospace for code-tab labels; a 7-step rounded scale running 6px / 8px / 12px / 16px / 24px / 48px / 60px-pill / circle; a 9-step spacing scale anchored at the 4px base unit and topping out at the 96px section rhythm; and 19 component definitions covering the dark top-nav, search field, primary and secondary buttons, the pill segmented switcher, code-tab IDE surface, customer-story card, Copilot prompt card, workflow-runs table, and the customer logo strip.

    Feed the file to Claude, Cursor, or Copilot and the agent reproduces GitHub's restraint — single arctic-link-blue voltage, dual-canvas band switching, weight-460 h2 instead of bold display, pill-radius segmented controls, two greens with split duty between glow and button — rather than a generic dark-mode dev landing with a glowing brand-purple CTA. Or reference the tokens directly: every hex, font name, radius, and spacing value is a quoted scalar you can paste into Tailwind config, CSS variables, or a shadcn theme registry. GitHub is worth studying because it threads a hard needle: a console-feeling surface that still reads as a magazine page, with no chromatic CTA, no atmospheric gradient outside one Copilot moment, and a display ramp that hovers between weights 425 and 480 — never bold, never timid, always sitting in the variable-axis micro-band that Mona Sans was designed for.
  related:
    - href: "/design"
      title: "Browse all design systems"
      description: "The full directory of DESIGN.md files on shadcn.io, with live mockups for each."
    - href: "https://github.com"
      title: "GitHub — official site"
      description: "The live source — see the dual-canvas band switching, the pill segmented switcher, and the Copilot IDE mockup in their native habitat."
    - href: "https://github.com/google-labs-code/design.md"
      title: "The DESIGN.md specification"
      description: "Google Labs' open spec for machine-readable design system files — the format this page is built on."
  questions:
    - id: "primary-color"
      title: "What is GitHub's primary brand color?"
      answer: "GitHub's load-bearing chromatic token is an arctic pale-blue at \"#8dd6ff\" — count 138 across the inspected page, mapped to \"--brand-color-text-link-rest\" and \"--brand-InlineLink-color-rest\". It paints every link rest, the hero pill stroke, and the Copilot CTA label. The accent green \"#5fed83\" is reserved for activity-glow gradients and the Copilot CTA-banner shadow start — it never fills a button. The actual primary-action paint is success-emphasis green \"#08872b\", mapped to \"--button-primary-bgColor-rest\". Two greens, two jobs."
    - id: "dark-mode"
      title: "Does GitHub's marketing site have a light mode?"
      answer: "No — the public marketing canvas is dark-only. The page floor is \"#0d1117\" (the canonical product dark-mode surface), with full-bleed \"#000000\" bands cutting in at hero and CTA section boundaries. Surface lift comes from \"#151a22\" cards and \"#24292f\" buttons. The product itself ships a light theme inside the app surfaces shown in mockups, but it is not represented in this DESIGN.md, which captures the public dark marketing experience."
    - id: "typography"
      title: "What typography does GitHub use, and what should I use if Mona Sans isn't available?"
      answer: "GitHub runs Mona Sans (variable) as the single sans family for display and body, with ui-monospace / SF Mono on every code tab and lineNumber surface. The hero hits weight 425 at 64px with -2.24px letter-spacing; h2 sits at weight 460 at 40px; h3 card titles at weight 480 at 22px — variable weights that land between regular and medium, never the bold 700 most dev platforms reach for. Mona Sans is open-source under SIL OFL, so substitution is rarely necessary. If you must, Inter at weights 400/500 with -1.5% tracking on display is the closest free fallback."
    - id: "shapes-and-depth"
      title: "What is GitHub's shape and elevation language?"
      answer: "Three radius tiers. Pill 60px (12 occurrences, the page's dominant value) carries the Code/Plan/Collaborate/Automate/Secure segmented switcher. Cards and the Copilot prompt panel sit at 16px (9 occurrences). CTA buttons hold 8px (6 occurrences) — a developer dialect rather than a consumer 12-14px feel. Depth comes from the dual-canvas band switch (\"#0d1117\" to \"#000000\") and the purple-to-indigo radial atmospheric behind the Copilot IDE mockup. The system uses near-zero drop shadows — surface lift carries the hierarchy."
    - id: "purple-glow"
      title: "What's the purple-to-indigo gradient for, and where can I use it?"
      answer: "The purple-to-indigo radial sits behind the Copilot IDE mockup as a single atmospheric moment — composed of \"#9350ff\" (vivid violet), \"#5049c2\" (indigo), \"#7873cb\" (muted lavender), and \"#0e0aa2\" (deep indigo) bleeding into \"#000240\" near-black. It is the page's only chromatic atmosphere and is strictly scoped to the Copilot region. Using it on workflow-runs cards, customer-logos, or the top nav would break the brand's single-glow rule. The activity-glow green \"#5fed83\" plays a parallel role on the workflow-runs region but never overlaps the same surface as the purple."
    - id: "use-in-project"
      title: "Can I use this DESIGN.md to build my own React developer-tools landing?"
      answer: "Yes — the file is designed to be fed into Claude, Cursor, or any AI tool that reads structured design tokens. The agent will reproduce GitHub's restraint: dark canvas, single arctic-link-blue voltage \"#8dd6ff\", variable-weight Mona Sans display, pill segmented controls, dual-canvas band switching. Reference the tokens directly inside Tailwind config or CSS variables — every color, type style, radius, and spacing value is a quoted scalar. The 19 component definitions cover the dark top-nav, search field, primary and secondary buttons, the pill segmented switcher, the code-tab IDE surface, the workflow-runs table, and the customer logo strip."

colors:
  canvas: "#0d1117"
  canvas-pure: "#000000"
  canvas-inset: "#010409"
  surface-1: "#151a22"
  surface-2: "#24292f"
  surface-3: "#2e374a"
  ink: "#ffffff"
  ink-default: "#f0f6fc"
  ink-muted: "#9198a1"
  ink-subtle: "#a4aea6"
  ink-neutral: "#7c8980"
  hairline: "#484f58"
  link-blue: "#8dd6ff"
  link-accent: "#4493f8"
  accent-green: "#5fed83"
  success-emphasis: "#08872b"
  danger-red: "#ff7b72"
  glow-violet: "#9350ff"
  glow-indigo: "#5049c2"
  glow-lavender: "#7873cb"
  glow-deep: "#0e0aa2"
  glow-plum: "#e6b7fe"

typography:
  display-hero:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, Segoe UI, Helvetica, Arial, sans-serif"
    fontSize: 64px
    fontWeight: 425
    lineHeight: 1.08
    letterSpacing: "-2.24px"
  display-h2:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 40px
    fontWeight: 460
    lineHeight: 1.2
    letterSpacing: 0px
  display-h2-emph:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 40px
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: 0px
  heading-card:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 22px
    fontWeight: 480
    lineHeight: 1.4
    letterSpacing: 0px
  heading-card-bold:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 22px
    fontWeight: 600
    lineHeight: 1.4
    letterSpacing: 0px
  body-lg:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 18px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0.18px"
  body-md:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0.24px"
  body-md-medium:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 16px
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: "0.24px"
  body-sm:
    fontFamily: "Mona Sans VF, -apple-system, system-ui, Segoe UI, Noto Sans, Helvetica, Arial, sans-serif"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.25
    letterSpacing: 0px
  caption:
    fontFamily: "Mona Sans VF, -apple-system, system-ui, Segoe UI, Noto Sans, Helvetica, Arial, sans-serif"
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0px
  button-md:
    fontFamily: "Mona Sans, MonaSansFallback, -apple-system, system-ui, sans-serif"
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0px
  code:
    fontFamily: "ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, Liberation Mono, monospace"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0px

rounded:
  none: "0px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
  xxl: "24px"
  xxxl: "48px"
  pill: "60px"
  circle: "9999px"

spacing:
  xxs: "2px"
  xs: "4px"
  sm: "8px"
  md: "12px"
  base: "16px"
  lg: "24px"
  xl: "48px"
  xxl: "64px"
  section: "96px"

components:
  top-nav:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md}"
    rounded: "{rounded.none}"
    height: 64px
    padding: "0 16px"
    border: "0"
  nav-link:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md}"
    rounded: "{rounded.none}"
    padding: "8px"
    height: 40px
  search-field:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.ink-muted}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.sm}"
    padding: "8px 12px"
    height: 32px
  button-primary:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.link-blue}"
    typography: "{typography.button-md}"
    rounded: "{rounded.md}"
    padding: "6px 28px"
    height: 56px
    border: "1px solid {colors.ink}"
  button-primary-hover:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.link-blue}"
    typography: "{typography.button-md}"
    rounded: "{rounded.md}"
    padding: "6px 28px"
    height: 56px
  button-success:
    backgroundColor: "{colors.success-emphasis}"
    textColor: "{colors.ink}"
    typography: "{typography.button-md}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
  button-secondary:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink}"
    typography: "{typography.button-md}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
    border: "1px solid {colors.hairline}"
  pill-switch:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.ink-muted}"
    typography: "{typography.body-md-medium}"
    rounded: "{rounded.pill}"
    padding: "8px 16px"
    height: 40px
  pill-switch-selected:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md-medium}"
    rounded: "{rounded.pill}"
    padding: "8px 16px"
    height: 40px
  code-tab-panel:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.ink-default}"
    typography: "{typography.code}"
    rounded: "{rounded.xl}"
    padding: "0"
    border: "1px solid {colors.surface-2}"
  copilot-prompt-card:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.ink-default}"
    typography: "{typography.body-md}"
    rounded: "{rounded.xl}"
    padding: "24px"
  workflow-runs-table:
    backgroundColor: "{colors.surface-1}"
    textColor: "{colors.ink-default}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.lg}"
    padding: "16px"
    border: "1px solid {colors.surface-2}"
  customer-story-card:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink}"
    typography: "{typography.body-lg}"
    rounded: "{rounded.none}"
    padding: "48px"
  customer-logo-strip:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink-subtle}"
    typography: "{typography.body-md}"
    rounded: "{rounded.none}"
    padding: "48px 0"
  text-input:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.canvas-pure}"
    typography: "{typography.body-md}"
    rounded: "{rounded.md}"
    padding: "18px 12px 0 18px"
    height: 46px
  cta-banner:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink}"
    typography: "{typography.display-h2}"
    rounded: "{rounded.none}"
    padding: "96px 0"
  accent-dot:
    backgroundColor: "{colors.accent-green}"
    textColor: "{colors.canvas-pure}"
    typography: "{typography.caption}"
    rounded: "{rounded.circle}"
    height: 8px
  link-text:
    backgroundColor: "transparent"
    textColor: "{colors.link-blue}"
    typography: "{typography.body-md}"
    rounded: "{rounded.none}"
    padding: "0"
  footer:
    backgroundColor: "{colors.canvas-pure}"
    textColor: "{colors.ink-muted}"
    typography: "{typography.caption}"
    rounded: "{rounded.none}"
    padding: "64px 48px"
---

## Overview

GitHub's marketing canvas is a console-feeling surface wrapped in editorial restraint — `{colors.canvas}` ("#0d1117") on the product-mirroring sections, with full-bleed `{colors.canvas-pure}` ("#000000") bands cutting in for hero openers and the Copilot CTA region. Body and headline ink hold "#ffffff" against the dark; the load-bearing chromatic token is an arctic pale-blue `{colors.link-blue}` ("#8dd6ff") that lights every link rest, hero pill stroke, and Copilot CTA label across the page. Where most developer platforms reach for a saturated brand purple or electric green as primary voltage, GitHub keeps the link blue dominant and reserves `{colors.accent-green}` ("#5fed83") for activity-glow gradients only.

**Two-greens-two-jobs**: the system runs two distinct green tokens with non-overlapping roles. Accent green `{colors.accent-green}` ("#5fed83") lights the workflow-activity glow and the Copilot CTA-banner shadow start — it never fills a button. Success-emphasis green `{colors.success-emphasis}` ("#08872b") carries the actual primary-action paint via `--button-primary-bgColor-rest`. This split is the single most distinctive token-architecture move in the system: separating "glow" from "button" prevents the bright lime from leaking into action surfaces where it would read as a status badge.

Display type runs **Mona Sans variable** at weights that land between regular and medium — never bold. The hero hits weight 425 at 64px with -2.24px letter-spacing for an editorial compression rather than a tech-bombastic shout; h2 sits at weight 460 at 40px with neutral tracking; h3 card titles at weight 480 at 22px. Unlike the convention of cranking dev-platform display to weight 700, GitHub uses weight variation in the 425-480 micro-band as the entire display strategy. The body family is Mona Sans VF at 14-18px; code tabs and lineNumbers run ui-monospace / SF Mono.

**Pill-as-segmented-control**: the Code/Plan/Collaborate/Automate/Secure switcher rides a 60px pill radius (the page's dominant radius value at 12 occurrences) over a `{colors.surface-1}` track. Cards land at 16px corners (9 occurrences); CTA buttons hold 8px (6 occurrences) — a developer dialect. Depth comes from the dual-canvas band switch and the purple-to-indigo radial atmospheric behind the Copilot IDE mockup, composed of `{colors.glow-violet}` ("#9350ff"), `{colors.glow-indigo}` ("#5049c2"), `{colors.glow-lavender}` ("#7873cb"), and `{colors.glow-deep}` ("#0e0aa2"). The glow is the page's only chromatic atmospheric — strictly scoped to one Copilot moment.

**Key Characteristics:**
- Dual-canvas system — `{colors.canvas}` ("#0d1117") for product-surface sections and `{colors.canvas-pure}` ("#000000") for hero and CTA bands.
- Single chromatic link voltage — `{colors.link-blue}` ("#8dd6ff"), count 138, mapped to `--brand-color-text-link-rest`.
- Two greens, two jobs — `{colors.accent-green}` for activity-glow gradients only; `{colors.success-emphasis}` for the primary-action paint.
- Variable-weight Mona Sans — display runs 425-480, never bold 700.
- 60px pill radius dominates — the Code/Plan/Collaborate/Automate/Secure segmented switcher.
- Single chromatic atmospheric — the purple-to-indigo Copilot glow, scoped to one region.
- Code-tab and IDE mockups in `{colors.surface-1}` ("#151a22") panels with 16px corners; chrome stays minimal so the product captures carry the page.

## Colors

The palette reads as a structural ladder around the dark canvas plus one arctic-blue voltage and two greens with split duty.

- **Canvas (`#0d1117`)** — frequency 6. Used as bg (6). The product-mirroring section floor; mapped as `--bgColor-default`, `--brand-color-canvas-subtle`, `--codeMirror-bgColor`. The single most-named surface variable in the page.
- **Canvas pure (`#000000`)** — frequency 18. Used as bg (13), text (2), border (2). Full-bleed hero and CTA bands; mapped as `--brand-SubNav-bgColor`, `--brand-color-canvas-default`. The band-switch partner that punches through `{colors.canvas}` at section boundaries.
- **Canvas inset (`#010409`)** — count 0 in the visible viewport, but mapped to `--bgColor-black`, `--overlay-bgColor`, `--dashboard-bgColor`. The deepest dark, reserved for overlays and the dashboard inset.
- **Surface 1 (`#151a22`)** — frequency 3. Used as bg (2), border (1). Card and panel surface for code tabs and the Copilot prompt; mapped as `--card-bgColor`, `--header-bgColor`, `--bgColor-muted`.
- **Surface 2 (`#24292f`)** — frequency 10. Used as bg (2), text (2), border (6). Button-default rest and hovered control surface; mapped as `--button-default-bgColor-rest`, `--control-bgColor-rest`.
- **Surface 3 (`#2e374a`)** — frequency 1. Used as bg (1). Active-state lift for default buttons; mapped as `--button-default-bgColor-active`, `--control-bgColor-active`.
- **Ink (`#ffffff`)** — frequency 677. Used as text (346), border (319), gradient (5), bg (7). The dominant text and hairline token; mapped to over 80 brand and component variables. Carries headlines, body, and the primary-button border-rest stroke.
- **Ink default (`#f0f6fc`)** — frequency 135. Used as text (70), border (65). The CodeMirror foreground and diff-addition-line text; mapped as `--fgColor-default`, `--codeMirror-fgColor`.
- **Ink muted (`#9198a1`)** — frequency 156. Used as text (78), border (78). Caption ink and disabled-control color; mapped as `--fgColor-muted`, `--control-fgColor-placeholder`. The workhorse secondary text.
- **Ink subtle (`#a4aea6`)** — frequency 96. Used as text (48), border (48). Subheadline and eyebrow ink on dark; mapped as `--brand-color-text-muted`, `--brand-color-text-subtle`.
- **Ink neutral (`#7c8980`)** — frequency 4. Used as text (2), border (2). The "neutral emphasis" gray; mapped as `--brand-color-neutral-emphasis`, `--brand-color-neutral-muted`.
- **Hairline (`#484f58`)** — frequency 2. Used as border (2). The strongest hairline rule; mapped as `--display-gray-scale-3`. Carries form-field and card divider strokes.
- **Link blue (`#8dd6ff`)** — frequency 138. Used as text (70), border (68). The page's load-bearing chromatic voltage; mapped as `--brand-color-text-link-rest`, `--brand-InlineLink-color-rest`, `--brand-Link-color-accent`. Reserved for link rests, the primary-button label, and the hero pill stroke.
- **Link accent (`#4493f8`)** — count 0 in viewport, mapped to `--fgColor-link`, `--fgColor-accent`, `--brand-color-focus`. The product-surface link blue (vs. the marketing arctic blue) — appears inside the IDE mockups.
- **Accent green (`#5fed83`)** — frequency 5. Used as text (2), border (2), gradient (1). The activity-glow voltage; mapped as `--brand-color-accent-primary`, `--brand-CTABanner-shadow-color-start`, `--brand-tiles-highlightColor`. Never fills a button.
- **Success emphasis (`#08872b`)** — frequency 2. Used as bg (2). The primary-action paint; mapped as `--button-primary-bgColor-rest`, `--bgColor-success-emphasis`. The token that actually carries `bgColor` on the green CTA button.
- **Danger red (`#ff7b72`)** — count 0 in viewport, mapped to `--brand-color-text-danger`, `--brand-color-text-error`, `--color-prettylights-syntax-keyword`. Error text and syntax-keyword highlighting inside code surfaces.
- **Glow violet (`#9350ff`)** — frequency 2. Used as gradient (2). The brightest stop in the Copilot purple atmospheric.
- **Glow indigo (`#5049c2`)** — frequency 3. Used as gradient (3). The mid-tone of the Copilot radial.
- **Glow lavender (`#7873cb`)** — frequency 2. Used as gradient (2). The transitional lavender between violet and indigo.
- **Glow deep (`#0e0aa2`)** — frequency 1. Used as gradient (1). The deep-indigo edge of the Copilot atmospheric; mapped as `--base-color-scale-blue-9`.
- **Glow plum (`#e6b7fe`)** — frequency 3. Used as gradient (3). The pink-violet top edge of the Copilot radial; mapped as `--display-plum-scale-9`.

## Typography

A single sans family plus one mono — Mona Sans variable does all the lifting from 64px hero down to 12px caption, with ui-monospace scoped to code tabs and lineNumbers.

**Mona Sans (variable)** is the single sans family. The hero hits weight 425 at 64px with -2.24px letter-spacing — a variable-weight cut that lands between regular (400) and medium (500), compressed by aggressive negative tracking for editorial density rather than tech-bombastic shout. H2 sits at weight 460 at 40px with neutral tracking ("Accelerate your entire workflow"). H3 card titles run weight 480 at 22px. Body large is 18px weight 400 with 0.18px positive tracking (paragraph copy under hero). Body medium is 16px weight 400, used on buttons, nav links, and label captions. Mona Sans VF (the variable-only cut) handles 14px nav and link surfaces. Mona Sans ships under the SIL Open Font License, so substitution is rarely necessary — Inter at weights 400/500 with -1.5% tracking on display is the closest free fallback.

**ui-monospace / SF Mono** runs every code-tab header and CodeMirror line in the IDE mockup at 14px weight 400. The mono is scoped exclusively to code surfaces — it never carries body, nav, or button content. JetBrains Mono or Geist Mono substitute cleanly.

The signature typographic move is **weight 425-480 as the entire display range** — no weight 700 anywhere on the marketing canvas. Where most dev platforms crank display bold to compete with hero illustrations, GitHub uses Mona Sans's variable-weight cut to land tiny gradations between 400 and 500. The 64px hero at weight 425 reads as a magazine headline rather than a SaaS callout.

## Layout

The system runs on a 4px base unit: 2, 4, 8, 12, 16, 24, 48, 64, 96. The 96px value handles vertical section padding — the page rhythm is generous, with section bands switching canvas color rather than just adding more whitespace. Card internal padding holds 24px consistently, giving content room to breathe inside `{colors.surface-1}` panels. The container strategy alternates: hero text caps at a reading column (~640-924px), while the Copilot IDE mockup spans wider to let the code tabs breathe. Customer logo strips run 48px of vertical padding inside their own canvas band, separating them from adjacent sections through canvas color rather than gap.

## Elevation & Depth

Elevation is achieved through **canvas band switching and one chromatic atmospheric** — not through shadows. The marketing surface carries near-zero box-shadows on cards or buttons. Hierarchy lifts via the dual-canvas swap (`{colors.canvas}` ↔ `{colors.canvas-pure}`) at section boundaries, and via the surface ladder running from `{colors.canvas}` up through `{colors.surface-1}`, `{colors.surface-2}`, `{colors.surface-3}`. The Copilot IDE mockup sits over the page's only true atmospheric — the purple-to-indigo radial composed of `{colors.glow-violet}` through `{colors.glow-deep}` with `{colors.glow-plum}` at the top edge. The workflow-runs region uses `{colors.accent-green}` as a parallel single-color glow. No drop-shadow tokens, no elevation tiers, no soft-edge cards floating over canvas.

## Shapes

A three-tier radius scale. **Pill 60px** (the page's dominant radius at 12 occurrences) carries the Code/Plan/Collaborate/Automate/Secure segmented switcher and a few wide CTA shapes. **Card 16px** (9 occurrences) covers the Copilot prompt card, the code-tab IDE panel, and feature surfaces. **Button 8px** (6 occurrences) holds the primary Copilot CTA at 56px height and most action buttons — a developer dialect rather than the consumer 12-14px feel. The 24px tier (7 occurrences) handles the search-field hairline and a few intermediate surfaces. Full-circle 9999px is reserved for activity dots and avatar masks. Everything else stays at 0px — full-bleed bands have no rounded corners, the wordmark sits flat, and the customer-logo strip is flush-edged.

## Components

The component vocabulary covers the dark top-nav, the search field, three button variants, the pill segmented switcher, the code-tab IDE panel, the Copilot prompt card, the workflow-runs table, customer-story and logo strips, the email-capture text input, the CTA banner, and link text. The **top-nav** sits on `{colors.canvas-pure}` at 64px height with `{typography.body-md}` nav links. The **button-primary** is the headline Copilot CTA: `{colors.surface-2}` background, `{colors.link-blue}` label, white 1px stroke, 8px radius, 6px/28px padding, 56px height — the page's most visually-loaded interactive element. The **button-success** carries `{colors.success-emphasis}` for the actual primary-action paint elsewhere on product surfaces. The **pill-switch** runs `{rounded.pill}` 60px with `{colors.surface-1}` track and a `{colors.surface-2}` selected state. The **code-tab-panel** holds `{rounded.xl}` 16px corners over `{colors.surface-1}` with a `{colors.surface-2}` hairline. The **copilot-prompt-card** mirrors the panel geometry but for the chat-style mockup. The **customer-story-card** sits on a full-bleed `{colors.canvas-pure}` band at 48px padding, no radius — the band itself is the card.

## Do's and Don'ts

- Do hold `{colors.link-blue}` ("#8dd6ff") for every link rest, the primary-button label, and the hero pill stroke. This is the single load-bearing chromatic voltage; count 138 across the page.
- Don't use `{colors.accent-green}` ("#5fed83") as a button background. It is mapped to `--brand-color-accent-primary` and the CTA-banner shadow gradient — pure glow duty. For green primary actions, use `{colors.success-emphasis}` ("#08872b") via `--button-primary-bgColor-rest` instead.
- Do switch canvas color at section boundaries between `{colors.canvas}` ("#0d1117") and `{colors.canvas-pure}` ("#000000"). The dual-canvas band switch is the page's primary hierarchy signal — using a single canvas color flattens every section.
- Don't crank display weight to 700. Mona Sans hero sits at weight 425, h2 at weight 460, h3 at weight 480. The variable-weight micro-band is the entire display strategy — bold display breaks the editorial register.
- Do reserve the purple-to-indigo radial (`{colors.glow-violet}` → `{colors.glow-indigo}` → `{colors.glow-lavender}` → `{colors.glow-deep}` with `{colors.glow-plum}` at the top edge) for the Copilot region only. It is the page's single chromatic atmospheric.
- Don't apply the Copilot glow to workflow-runs cards or customer-logo strips. The workflow region carries its own activity-green glow via `{colors.accent-green}`; mixing glows breaks the single-atmospheric rule.
- Do use 60px pill radius for segmented switchers and 8px for action buttons. The pill is the page's dominant radius (12 occurrences); the 8px button corner is the developer-dialect default.
- Don't use 12px or 24px radius on action buttons — those tiers belong to the search field hairline and intermediate card surfaces. Mixing radius tiers across components breaks the three-tier discipline.
- Do use ui-monospace / SF Mono for every code-tab header and CodeMirror lineNumber. The mono is strictly scoped to code surfaces — Mona Sans body in a code tab reads as a marketing label, not a file path.

## Known Gaps

- Light mode is not documented because the public marketing surface is dark-only. The product itself ships a light theme inside the in-app surfaces shown in mockups; those tokens are not represented here.
- Motion timings (the Copilot glow pulse, the workflow-runs activity sweep, the carousel auto-advance) are out of scope — only static surface tokens captured.
- The `Mona Sans` and `Mona Sans VF` families are open-source under SIL OFL — no licensed substitute required, but the variable-axis weight values (425, 460, 480) require a variable-font runtime.
- Form-field error and validation states beyond the email-capture input are not visible on the inspected marketing surface.
- The danger red `{colors.danger-red}` ("#ff7b72") and link accent `{colors.link-accent}` ("#4493f8") are mapped to product variables (`--color-prettylights-syntax-keyword`, `--fgColor-link`) but appear inside IDE mockups rather than as marketing-chrome tokens.
- The customer-logo strip is captured as a marketing region; the underlying logo SVGs are external brand assets, not GitHub design tokens.
