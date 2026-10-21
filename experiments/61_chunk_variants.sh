#!/bin/bash
# Chunking comparisons on the real documents: each variant builds its own index, then is evaluated
# with the default search options. Strictly sequential. Progress goes to /rag/results/61_chain.log
log=/rag/results/61_chain.log
step() { echo "$(date +%H:%M:%S) $*" >> "$log"; }
ev() { bash /rag/experiments/59_eval_one.sh "$@" > /dev/null; }
variant() {
    tag=$1; shift
    step "index $tag $*"
    bash /rag/experiments/54_index_real.sh "$tag" "$@" > /dev/null
    step "eval $tag"
    ev "$tag" - "$@"
}
: > "$log"
# search options on the 256-token index, with the final defaults (no agreement step)
step "b256 extra evals"
ev b256 -
ev b256 _a0_offset0 --image-offset 0
ev b256 _a0_offset004 --image-offset 0.04
ev b256 _a0_offset012 --image-offset 0.12
ev b256 _noimages --no-image-search
ev b256 _cand20 --candidates 20
variant b128 --max-tokens 128
variant b512 --max-tokens 512
variant bnosplit --no-heading-split
variant bplain --no-headings
step "done"
