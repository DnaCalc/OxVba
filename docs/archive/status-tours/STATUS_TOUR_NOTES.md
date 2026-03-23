# Status Tour Notes

Directory purpose:
- Store dated showcase/orientation tours of the current OxVBA implementation state.

Pattern to follow:
- Use a timestamped filename: STATUS_TOUR_yyyy-MM-dd_HHmmss.md.
- Make it practical and narrative, not only abstract architecture.
- Include local code snippets from current files.
- Include live command outputs for working and failing examples.
- Clearly separate "implemented now" vs "scaffold/deferred".
- Include concrete answers to:
  - What compiles?
  - What runs?
  - What does syntax/green/red look like today?
  - Is IL/bytecode in place and executable?
- Link or cite current gate/evidence artifacts when relevant.

Original user prompt that created the first tour:
"Make me a little tour (in a tmp\STATUS_TOUR_<date>.md that shows fragments of the code and project now in place, examples of what compiles and what the green/red tree strucutre for that code looks like today, is there the IL and interpreter in place for this (can it run?) I'm struggling to underestand what has been done. Make it a showcase and orientation doc for the story so far. I should be pretty and need not be short - rather make comprehensive in scope, with local snippets to give insight."