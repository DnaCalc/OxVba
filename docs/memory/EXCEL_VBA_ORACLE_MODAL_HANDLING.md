# Excel/VBA Oracle Modal Handling Memory

Source: Govert's Excel/VBA Agentic Coding Guide and Jun 27, 2026 follow-up:
https://gist.github.com/govert/2d3946830c35c74806df3f32b597eb72

When driving real Excel/VBA from an agent, always assume compile/runtime modal
dialogs may appear and block COM automation. Have a UI Automation watcher/helper
ready before running VBA. If Excel is not responsive after one or two seconds,
inspect UIA windows scoped to the owned Excel/VBE process and capture:

- modal dialog text,
- highlighted VBE token,
- full selected code line via `TextPattern.GetSelection()` and line expansion,
- visible button names before dismissing anything.

For compile diagnostics, do not use `Application.Run` as the compile check.
Make the VBE visible, invoke Debug -> Compile VBAProject, then read the modal
with UIA. `Application.Run` can return the generic "macro may not be available"
message when a module failed to compile, even if the target macro exists and
macros are enabled.

Treat "Cannot run the macro ... may not be available" as ambiguous among:
macros disabled, missing macro name, or compile failure somewhere in the project.
If `AccessVBOM=1`, macros are enabled, and the macro exists, assume compile
failure until proven otherwise.

Error location is not always defect location. "Sub or Function not defined"
often highlights the call site; also inspect the called procedure declaration
and watch for intrinsic-name shadowing such as `Fix`, `Date`, `Time`, `Name`,
`Error`, `Left`, `Right`, `Len`, `Val`, and `Format`.

Keep cleanup scoped: only dismiss dialogs owned by the Excel/VBE process for the
current oracle run, and only terminate Excel PIDs recorded as owned by that run.
