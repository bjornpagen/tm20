# Representation First

The data representation determines the program’s complexity. Algorithms and
control flow are downstream of it.

Every change to this repository begins with a representation verdict:

1. What data, type, relation, or invariant makes the bad state inexpressible?
2. What proof is learned at the boundary, and which type carries it?
3. Which variation is essential and therefore belongs behind polymorphic
   dispatch?
4. Which policy is better represented as inspectable data plus a small
   evaluator?
5. If representation cannot remove the branch, why is the complexity
   essential rather than accidental?

## Laws

- Parse, do not merely validate. A successful boundary returns a type that
  preserves what was learned.
- Prefer disjoint unions to independent flags.
- A provider switch is polymorphism not yet named.
- Absence and uncertainty are real variants, never magic values.
- Use half-open ranges for time and capacity windows.
- Reify complicated policy as data before it grows an accidental interpreter.
- Keep nominal identities nominal even when their physical representation is
  the same integer or digest.
- Erase types once, at the runtime plugin boundary; retain precise associated
  types on each side.

## Application here

- Provider-native cursors, events, effects, and errors live in
  `TypedConnector`; `ErasedConnector<T>` alone produces `dyn Connector`.
- `Observation`, `Notice`, `Edition`, `DeliveryAttempt`, and `EffectIntent`
  are different relations because they mean different things.
- Bumbledb mirrors require exact evidence arms for every delivery and effect
  phase.
- A transport disconnect after transmission is `Ambiguous`, not a failed
  boolean.
- A reprint is a new attempt linked to a terminal original with a reason.
- Printing may release an upstream effect, but it never becomes human-read or
  upstream-read by alias.
- Policy tables are parsed once into a total, safe `PolicyTable`.
- Paper text is parsed and privacy-projected before rendering.

## Limit

Representation removes accidental complexity, not essential complexity.
Forcing genuinely different providers or delivery outcomes into one shape
only hides branching in nullable fields and capability flags. The local cost
of another type or relation is justified only when it removes repeated checks
or preserves a real invariant globally.

The practitioner lineage is Brooks → Pike → Raymond → Torvalds. The mechanism
is sharpened by Minsky’s “make illegal states unrepresentable,” King’s “Parse,
Don’t Validate,” Reynolds and Wadler’s parametricity, Dijkstra’s half-open
coordinates, and SICP’s control-flow-as-data ceiling.
