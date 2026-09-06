// A split-on-Enter produces a second statement the backend knows nothing about:
// there is no `splitStatement`/`createStatement` mutation, only `editStatement`
// keyed by an existing uid. We tag synthetic uids with this marker so the
// persistence seam can recognise --- and skip --- statements that would 404.
// BACKEND GAP: a `splitStatement(uid, atOffset)` mutation would let these persist.
export const SYNTHETIC_UID_MARKER = "::split-";
