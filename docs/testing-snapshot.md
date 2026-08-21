# Snapshot testing guide

This guide is for someone testing a development snapshot build of the Retonr
command-line interface. It explains exactly what the build does today, what it
deliberately refuses to do, and what feedback is useful.

Read [Current state](current-state.md) for the authoritative list of implemented
behavior. This guide summarizes it for a hands-on session and does not extend it.

## What you are testing

Retonr is a local-first editorial engine. The snapshot exposes one useful
workflow: **fidelity checking**.

You bring two documents. The first is your original. The second is a complete
rewritten version of it, produced by any tool or by hand. Retonr answers one
question: did the rewrite change anything that was supposed to stay fixed?

It checks quantities, URLs, email addresses, identifiers, declared protected
terms, document structure, newline shape, and unsafe control characters. If the
rewrite preserved all of them, Retonr reports `rewritten` and can write the
accepted bytes out. If it did not, Retonr reports `abstained` with a reason and
gives you back the exact original bytes instead.

The snapshot does not generate text. It never contacts the network.

The source repository also contains a development-only
`rewrite-eval --ollama-bound-preflight` command. It is not part of this snapshot CLI
workflow. When a developer explicitly supplies a versioned plan, that command may
contact only its IP-literal loopback Ollama endpoint. It sends the complete read-only
preflight over one directly connected retained HTTP/1 transport and checks native
connection attribution before traffic and after every fully drained response. It
does not generate, acquire, activate, qualify, or authorize a model. macOS refuses
the command before HTTP. Successful Windows and Linux reports remain unqualified and
explicitly do not prove exclusive socket ownership or application-handler execution.

The same snapshot also administers exact local model artifacts offline. These
commands copy, inspect, migrate, or remove local files. They do not download,
qualify, activate, or run a model:

```console
retonr --data-dir <DIRECTORY> model import <ARTIFACT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model import-set <SOURCE_ROOT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model inventory
retonr --data-dir <DIRECTORY> model inventory-set
retonr --data-dir <DIRECTORY> model pending-operations
retonr --data-dir <DIRECTORY> model migrate --yes
retonr --data-dir <DIRECTORY> model reconcile --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model reconcile-set --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model remove --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-removal --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model remove-set --artifact-set-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-set-removal --artifact-set-id <SHA256> --installation-generation <N> --yes
```

## Setup

1. Download the archive for your platform and verify its SHA-256 digest against
   the value in the release notes.
2. Extract it. The binary is `retonr` on Linux and macOS, `retonr.exe` on
   Windows.
3. On macOS the binary is unsigned, so Gatekeeper will refuse it until you clear
   the quarantine attribute yourself. Only do this if you verified the digest.
4. Run `retonr --version` to confirm it starts.

There is no installer. Do not put the binary on a path where other software will
pick it up automatically.

## The core loop

Check a rewrite held in two files:

```console
retonr check original.txt rewritten.txt --format text
```

Pipe the rewrite in instead, and keep the safe result:

```console
retonr check original.txt - --output checked.txt --format text
```

Either document may be `-`, meaning standard input, but not both. Standard input
is read to end of file without trimming, so blank lines, indentation, and a
missing final newline all survive exactly.

Get machine-readable output for scripting:

```console
retonr check original.txt rewritten.txt
```

Make a failed check stop a pipeline:

```console
retonr check original.txt rewritten.txt --fail-on-abstain
```

## Reading the result

`status` is one of:

| Status | Meaning |
| --- | --- |
| `rewritten` | The candidate passed every gate and was accepted |
| `abstained` | A gate failed, so the exact original is returned instead |
| `unchanged_no_eligible_content` | There was nothing eligible to change |

When `status` is `abstained`, `reason` explains which gate failed. The common
ones are `protected_value_changed` (a number, URL, email, identifier, or declared
term differs), `structure_changed` (paragraph or newline shape differs), and
`unsafe_text` (the candidate introduced control characters).

An abstention is a success, not an error. The command still exits `0` unless you
passed `--fail-on-abstain`. Exit `2` means your invocation was wrong, `3` means a
policy refusal, `4` means an input exceeded a limit.

## Output safety

`--output` writes to a **new** file. It refuses to replace an existing file and
never modifies your source. `--output -` sends the document to standard output
and moves the report to standard error so the two never mix.

Writing exact unescaped document bytes to a terminal requires `--raw-terminal
--yes` together. Either flag alone, or neither flag, writes escaped rendering
that cannot drive the terminal. This exists because untrusted text can carry
terminal control sequences.

## What will frustrate you, and why

These are known and expected. Reporting them again is not useful.

- **Almost any real paraphrase abstains.** The current evaluator accepts only
  literal, token-preserving changes such as punctuation and capitalization.
  Rewording a sentence is correctly rejected as an unverifiable change. The
  calibrated semantic evaluator that would accept paraphrases is later work.
- **The reason does not say which value changed.** Reports are content-redacted
  by policy. `--diff` shows an escaped line comparison; it does not name the
  protected value that failed.
- **There is no rewrite command.** The snapshot validates a candidate you supply.
  It does not produce one.
- **Plain text only**, UTF-8, up to 16 MiB. Markdown and DOCX are later phases.
- **No profiles, no style learning, no editorial lint.**

## What feedback is genuinely useful

In rough priority order:

1. **A wrong verdict.** A candidate that Retonr accepted but which actually
   changed a fact, quantity, link, or meaning. This is the most valuable report
   you can make. Include both documents if you can share them.
2. **A wrong abstention.** A candidate that only changed punctuation, spacing,
   or capitalization but was still rejected.
3. **Byte damage.** Any case where `--output` produced bytes that differ from
   the accepted candidate, or where an abstention did not return your original
   exactly. Compare with `cmp` or `fc`.
4. **Crashes, hangs, or panics**, especially on unusual input: very long lines,
   mixed newline styles, a byte order mark, unusual Unicode, or a file with no
   final newline.
5. **Platform behavior.** Long paths, locked files, read-only directories, paths
   with non-ASCII characters, and running under a pipe or redirect.
6. **Confusing output.** Places where the text report or an error message left
   you unsure what happened or what to do next.

Please do not report missing features that this guide already lists as not
implemented.

## How to report

Open an issue on the repository using the snapshot feedback template. Include:

- The tag and the commit from `BUILD-PROVENANCE.txt` in your archive.
- Your operating system and architecture.
- The exact command line you ran.
- What you expected and what happened.
- The `--format json` output where relevant.

Use synthetic or non-sensitive documents. Do not attach confidential material,
credentials, or personal data.

For a suspected security issue, do not open a public issue. Follow
[SECURITY.md](../SECURITY.md) and use GitHub private vulnerability reporting.

## Scope reminder

This snapshot is not a milestone release. No tagged version is supported for
production use. No schema, exit code, flag name, or output shape is frozen, and
any of them may change.
