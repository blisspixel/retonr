# Optional sister: fitr

[fitr](https://github.com/blisspixel/fitr) is a separate product: device-aware
evaluation of local models on the machine that will run them. **Retonr does
not require it.** There is no dependency, no download, and no call into fitr
from a rewrite.

The two questions stay distinct:

| | fitr | retonr |
| --- | --- | --- |
| Asks | is this model any good on this machine? | can this exact stack reconstruct this draft without breaking claims? |
| Unit | one model, one device fingerprint | one artifact digest, one runtime build, one hardware class |
| Verdict | PASS, FAIL, SKIP, n/a, or BLKD on independent needs | accept the candidate, or keep the original |
| Does not | qualify, activate, or license a model for retonr | rank models on a public leaderboard |

fitr can export `fitr.retonr.evidence.v1` with `fitr export <model> --retonr`.
That file is device-measurement evidence. It is not a qualification.

## How to read a result

```console
fitr run qwen3:30b --full
fitr export qwen3:30b --retonr
retonr model fitr ~/.fitr/results/qwen3:30b.retonr.json
```

`retonr model device-evidence` is the same command. It does not need
`--data-dir`. It does not create a repository, start a runtime, or change
qualification state. `qualified` remains `false` and `qualification` remains
`absent`.

Host names, driver strings, config paths, and result paths stay out of the
report. Need states that fitr did not measure stay absent.

## What retonr will not do

- Install, start, or configure fitr
- Treat a PASS scorecard as authority to rewrite
- Emit qualification-v2 records from fitr evidence
- Scan a home directory for fitr results
- Fail because fitr is absent
