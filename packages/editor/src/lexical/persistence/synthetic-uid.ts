// A split-on-Enter produces a second statement the backend knows nothing about
// until `createStatement` answers with a real uid. We tag synthetic uids with
// this marker so the persistence seam can recognise --- and skip --- statements
// that `editStatement`/`destroyStatement` would 404 on.
// BACKEND GAP: a `splitStatement(uid, atOffset)` mutation would let these persist.
export const SYNTHETIC_UID_MARKER = "::split-";
