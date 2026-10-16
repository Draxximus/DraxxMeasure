# Session and bundle lifecycle

The methods in core enforce local transitions. Uniqueness, open counts, request
identity, and current stored state are authoritative only inside a transaction.
They must not be inferred from a CLI counter or trusted from stale UI state.

## Normal flow

```text
Create Active session
  → select/create Part Number and immutable specification
  → add/select InspectionItem (repeat for other Part Numbers as needed)
  → start Open bundle → record committed sequential measurements
  → close bundle → start another bundle/item (repeat)
  → review → close session → obtain snapshot → analyze/export
```

## Session transitions

| Current | Operation | Result/guard |
| --- | --- | --- |
| Not created | Create | Active; durable ID and creation time |
| Active | Suspend | Suspended; open bundle retained, no capture accepted |
| Suspended | Resume | Active; same IDs, sequence, snapshot references |
| Active | Close | Closed only if authoritative open-bundle count is zero |
| Closed | Resume/Suspend/Close | Invalid; no implicit reopen or repeated close |
| Suspended | Close | Invalid; resume then close bundles/session explicitly |

Repeated delivery of an already committed request returns its original receipt
through idempotency; that is not a new lifecycle transition. New duplicate close
commands fail. Empty sessions/items can close, but Phase 2 bundles must contain
1..42 active measurements when closed. Empty bundles remain open; do not invent
sample data or erase them to permit closure. All entry, item creation, and bundle
starts require Active. Part-level `/close` exits persisted selection context;
session closure requires that context to be exited as well.

## Bundles and recording

- Start: Active session, item belongs to it, nonblank label, next per-item sequence.
  Enforce at most **one open bundle per session** in the initial design, even when
  switching items. Domain `Bundle::start` alone cannot enforce this global rule.
- Record: Active session, Open bundle, matching item/specification ownership,
  valid exact input/interpretation/resolution, and fewer than 42 effective values.
  Reaching 42 never auto-closes. Allocate the next per-bundle index
  transactionally; persist immediately before returning success.
- Close: Open → Closed, matching Active session. Close is explicit and irreversible
  in this phase. A specification change cannot change an existing bundle's item.
- Correct/invalidate: Active session, explicit target original, matching latest
  event and nonblank reason. Closed bundles may be corrected, but not appended to
  or invalidated down to zero. Restoring an invalidated value cannot exceed 42.
- Invalid: capture without a bundle, in Suspended/Closed sessions, in a Closed
  bundle, or using another item's/specification's context. No partial state change.

The core's mutation methods return errors before changing state when a transition
fails. Their callers must still roll back **all** related database writes on failure.

## Interruption, recovery, and closure

Unexpected process exit does not run a reliable suspend hook. A crash may leave
the persisted session Active and a bundle Open; recovery presents that committed
context for explicit operator continuation. The selected item is now persisted
in `session_context` even when no bundle is open; if no item is selected, remain at
session level. Do not invent uncommitted UI selections. An explicit Suspended session needs
Resume. Uncommitted input/drafts are not saved observations and must not be
described as such. Durable draft recovery, if added, needs a separate marked model.

Resolve uncertain requests before new commands. A lost acknowledgement can mean
the measurement committed; use the same request ID and verify its payload. Repeated
equal measured values with different request IDs remain distinct valid observations.

Closing the session and its commit/audit event is atomic. Closure never silently
closes bundles, discards drafts, or deletes data. Report failure after closure does
not reopen the session or change observations. Exports can be retried from a fresh
consistent snapshot; each output records its own revision and export identity.
