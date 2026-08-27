# Emit one record per speech line: seq<TAB>speaker<TAB>text
# Drops blank lines and page furniture; renumbers so seq counts real speech only.
# Speaker is sticky: a turn continues until the next explicit TAG: prefix.
{
    line = $0
    gsub(/\r/, "", line)
    # strip leading indentation
    sub(/^[ \t]+/, "", line)
    sub(/[ \t]+$/, "", line)

    if (line == "") next

    # page furniture: "<Name> Interview   <page#>", bare dates, bare page numbers,
    # and tape markers that pdftotext floats into the margin
    if (line ~ /Interview[ \t]+[0-9]+[ \t]*$/) next
    if (line ~ /^(January|February|March|April|May|June|July|August|September|October|November|December)[ \t]+[0-9]{1,2},?[ \t]*[0-9]{4}[ \t]*[0-9]*[ \t]*$/) next
    if (line ~ /^[0-9]{1,3}$/) next
    if (line ~ /^Tape[ \t]+[IVX0-9]+[ \t]*$/) next
    if (line ~ /^(ACT UP Oral History Project|Interview of|A Program of MIX)/) next

    # leading "Tape I" / "Tape II" gutter marker glued to real speech
    sub(/^Tape[ \t]+[IVX]+[ \t]+/, "", line)

    # speaker tag?
    if (match(line, /^[A-Z]{2,3}:/)) {
        tag = substr(line, 1, RLENGTH-1)
        speaker = tag
        line = substr(line, RLENGTH+1)
        sub(/^[ \t]+/, "", line)
        if (line == "") next
    }

    seq++
    printf "%d\t%s\t%s\n", seq, (speaker == "" ? "?" : speaker), tolower(line)
}
