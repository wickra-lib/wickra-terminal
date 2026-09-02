// Profile histogram scaling, kept out of the SFC so it can be tested without
// mounting a component or a browser — the same reason ./layout is a module.

/** The largest finite bin, or 0 when a profile has produced nothing yet. */
export function peakOf(bins: number[]): number {
  return bins.reduce((max, bin) => (Number.isFinite(bin) && bin > max ? bin : max), 0)
}

/**
 * A bin's width as a percentage of the widest bin in its **own** profile.
 *
 * Scaled per profile rather than across the panel, which is the same decision
 * the TUI widget makes: two distributions on one panel measure different things
 * — traded volume and a count of time slots — and one shared scale would
 * flatten whichever has the smaller units into nothing.
 *
 * A profile that has produced no bins yet, or a bin that is not a positive
 * finite number, draws nothing rather than a bar of some arbitrary length.
 */
export function binWidth(value: number, peak: number): string {
  if (!(peak > 0) || !Number.isFinite(value) || value <= 0) {
    return '0%'
  }
  return `${(value / peak) * 100}%`
}
