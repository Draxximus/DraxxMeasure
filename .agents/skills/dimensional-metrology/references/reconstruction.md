# Abbreviated-entry contract

Read when designing or testing two-digit reconstruction. This is a candidate
policy for approval, not a claim that production reconstruction is already decided.

## Questions that must be settled

- Do two digits denote the last two integer millimetre digits, instrument ticks,
  or another explicitly defined scale? Nominal decimal places do not answer this.
- What physical/operational capture interval is justified independently of
  specification limits? Can operators capture measurements outside it in full mode?
- Are negative dimensions meaningful? What decimal separator and full-mode syntax
  are accepted? How are precision mismatch and unit conversion handled?
- Which context and algorithm version are persisted for replay?

## One deterministic candidate: integer-mm suffix within an approved interval

Only after agreeing on this mode:

1. Require exactly two ASCII digits `00` through `99`; retain leading zeros in raw
   input. Interpret their integer suffix as `d` in millimetres. Do not borrow the
   nominal's fractional part or pretend the input measured tenths of a millimetre.
2. Configure finite exact bounds `[A, B]` in millimetres as an independent capture
   interval and persist them with the policy version. Require A ≤ B. These bounds
   are neither LSL/USL nor statistical control limits. Explain them to the operator.
3. Candidate values are all `100*k + d` mm in `[A, B]`, for integer `k` in
   `ceil((A-d)/100)` through `floor((B-d)/100)`. Use checked exact arithmetic and
   defined floor/ceil division, including negative bounds if supported. Calculate
   cardinality first; do not iterate an unbounded range.
4. One candidate: return that value and interpretation provenance. Zero: request
   full entry or an explicit context change. More than one: display alternatives
   or request full entry. Never silently choose a nearest value or discard candidates
   because they are out of specification. Do not revise bounds invisibly.
5. Full mode uses a documented disjoint grammar (for example `=9368.0`, if approved)
   and validates dimensions directly. It must allow valid out-of-spec observations
   and observations outside the abbreviation interval, with appropriate feedback.
6. An operator resolution of ambiguity is recorded as an explicit choice; editing
   a recorded value creates a correction, not an in-place history rewrite.

## Reference scenarios (synthetic tests, not inspection records)

| Nominal/specification | Capture interval | Input | Required result under this candidate |
| --- | --- | --- | --- |
| 9368.9 mm, ±5 mm | [9300, 9399] mm | `68` | 9368 mm; limits 9363.9 and 9373.9 mm |
| Same | [9300, 9399] mm | `80` | 9380 mm, valid but out of specification |
| Same | [9300, 9499] mm | `68` | Ambiguous: 9368 and 9468 mm |
| Same | [9370, 9399] mm | `68` | No candidate; request full mode |
| 9999 mm, limits independently set | [9950, 10049] mm | `00` | 10000 mm; rollover handled |
| Same | [9900, 10100] mm | `00` | Ambiguous: 9900, 10000, 10100 mm |

Also test `01`, empty/one/three digits, Unicode digits, whitespace policy, negative
bounds if supported, overflow, interval edges, conflicting units, and full decimal
entries. Fixed bounds may be operationally inconvenient at a rollover; surface
that limitation instead of silently replacing this algorithm with nearest-nominal.

## Required decision record

Record policy identifier/version, approved use cases, unit and resolution,
grammar, bounds source, candidate algorithm, ambiguity/override flow, rounding,
examples/counterexamples, and replay fixtures. A different approved algorithm is
acceptable if equally explicit, deterministic, tested, and traceable.
