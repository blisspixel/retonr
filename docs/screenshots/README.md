# Screenshot policy

## Purpose

Screenshots prove and explain real behavior. They are not speculative mockups and do
not present planned functionality as implemented.

## Required captures

### CLI vertical slice

- Successful rewrite with a readable diff
- Abstention with concise gate reasons
- Structured status or trace inspection

### Desktop beta

- Rewrite workbench
- Accessible linear diff
- Profile evidence and rule editor
- Model manager

### Voice release candidate

- Local voice interview with editable transcript

## Capture requirements

- Capture from a clean passing release build.
- Use deterministic, non-sensitive fixtures created for documentation.
- Record the binary version, platform, theme, viewport, scale, and fixture ID.
- Use the same fixture when comparing platforms.
- Show platform-specific images when behavior or native controls differ materially.
- Crop only irrelevant operating-system chrome.
- Do not edit product content, validation states, or timing claims into the image.
- Remove usernames, machine names, paths, tokens, and unrelated applications.
- Provide useful alt text and a nearby textual explanation.
- Keep text readable at the rendered README size.
- Recapture an image when the documented behavior or layout changes.

## File naming

```text
cli-rewrite-<platform>.png
cli-abstain-<platform>.png
cli-trace-<platform>.png
desktop-workbench-<platform>.png
desktop-profile-<platform>.png
desktop-voice-interview-<platform>.png
```

Use `windows`, `macos`, or `linux` for the platform segment. A platform-neutral image
may omit the segment only when the behavior and rendering are verified equivalent.

## Publication gate

An image can be linked from the main README only when:

- The corresponding feature milestone has passed.
- The fixture is checked in.
- The capture metadata is checked in.
- Accessibility review confirms that the surrounding documentation does not rely on
  the image alone.
- Repository policy checks pass.
