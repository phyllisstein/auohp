# Pass 1: find timecode markers, build seq -> absolute-seconds anchors,
#         detecting tape resets and accumulating a tape offset.
# Pass 2: interpolate every line's absolute time between bracketing anchors.
# Output: seq<TAB>abs_seconds<TAB>speaker<TAB>text

FNR==NR {
    split($0, F, "\t")
    seq = F[1]; txt = F[3]
    nlines = seq
    if (match(txt, /[0-9][0-9]:[0-9][0-9]:[0-9][0-9]/)) {
        tc = substr(txt, RSTART, RLENGTH)
        split(tc, T, ":")
        within = T[1]*3600 + T[2]*60 + T[3]
        # tape reset: time went backwards -> start a new tape
        if (na > 0 && within < prev_within) {
            tape_offset += prev_within + 300   # prior tape ran ~5min past its last marker
        }
        prev_within = within
        na++
        aseq[na] = seq
        asec[na] = tape_offset + within
    }
    next
}

FNR==1 { i = 1 }
{
    split($0, F, "\t")
    seq = F[1]; spk = F[2]; txt = F[3]

    if (na == 0) { print seq "\t-1\t" spk "\t" txt; next }

    # advance to the anchor pair bracketing this seq
    while (i < na && aseq[i+1] <= seq) i++

    if (seq <= aseq[1]) {
        # before first marker: extrapolate backwards from anchor 1
        sec = asec[1] - (aseq[1] - seq) * (300.0 / (aseq[2] ? (aseq[2]-aseq[1]) : 60))
        if (sec < 0) sec = 0
    } else if (i >= na) {
        # after last marker: extrapolate forward
        span = (na > 1) ? (aseq[na] - aseq[na-1]) : 60
        sec = asec[na] + (seq - aseq[na]) * (300.0 / span)
    } else {
        lo = aseq[i]; hi = aseq[i+1]
        slo = asec[i]; shi = asec[i+1]
        frac = (hi > lo) ? (seq - lo) / (hi - lo) : 0
        sec = slo + frac * (shi - slo)
    }
    printf "%d\t%d\t%s\t%s\n", seq, sec, spk, txt
}
