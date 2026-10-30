# P011 desktop design system

The interface uses familiar native Settings interactions: a persistent sidebar,
one selected page, readable groups of facts and a shared report-status region.
It is original project source; no Apple source, proprietary icon, font or asset is
copied. The blue linked-segments SVG is project-owned under the repository MIT
license. Apple names identify interoperability goals, not affiliation.

Fusion Qt Quick Controls provide portable control behavior. Colors come from the
Qt application palette: window, alternateBase, base, text, mid and highlight.
There is no hardcoded light surface behind dynamic text. Platform palette support
controls automatic light/dark adoption; injected light/dark palettes are exercised
in the real-window smoke test. Desktop theme/portal behavior remains a manual
platform check. Status is written in words, not encoded by color alone.

The sidebar uses 205 px below a 1000 px window width and 250 px otherwise. Content
margins adapt from 22 to 36 px. Minimum size is 760×560; required 1280×720 and
2560×1440 sizes are smoke-tested. Long pages scroll vertically and wrapped labels
avoid horizontal scrolling. Cards use 12 px corners, 20 px padding, 14 px body
text and 17 px headings; the page heading is 29 px. All text uses system fonts.

Keyboard behavior: Tab/Shift+Tab reach navigation and actionable buttons; sidebar
Up/Down changes pages; Alt+Left/Right selects adjacent pages; Ctrl+R refreshes.
Item delegates and buttons retain Qt focus indicators and accessibility roles.
Sidebar names remain available as accessible names and hover tooltips when
elided. No unsupported operation appears as an enabled toggle. Refresh and Cancel
are real controls for the read-only report process.

A missing CLI explains how to install/select it while navigation remains usable.
Loading is explicit with a busy indicator and Cancel. Invalid JSON, process exit,
timeout and cancellation are distinguishable; stale rows have a shared warning
and last-success timestamp. A successful report is labelled a static build report,
never a discovered device or connected-hardware status. Devices explains the
missing adapter; Continuity explains absent v1 coverage. Research explicitly
separates source, software tests, Apple SDK compilation and unverified hardware.

Automated window tests cover all pages in light/dark and missing-CLI states,
keyboard navigation, Refresh, and size changes. A human must still evaluate
screen-reader output, long localized strings, desktop scaling and real
Wayland/X11 theme changes. Localization is not implemented in P011.
