# Workset / Epic / Bead Rollout Template

Use this template when a new active workset is accepted.

GPT-5.5 note: keep rollout content compact and outcome-first. Prefer clear
terminal conditions, completion evidence, and dependencies over procedural
detail that the execution agent can infer from repo context.

## 1. Workset

- Workset title:
- Workset doc path:
- Scope:
- Terminal condition:

## 2. Execution Epics

List the required execution epics under the workset.

Recommended baseline:
1. Workset initiation and epic rollout
2. First capability or reform lane
3. Second capability or audit lane
4. Validation / verification lane
5. Artifact retirement / demotion lane
6. First exposed gap fixes lane

For each epic:
- Epic title:
- Why it is a separate lane:
- Expected close condition:

## 3. Rollout Beads

For each epic, create the first rollout bead:

- Title:
  - `Roll out <epic-name> child beads`
- Outcome:
  - create or refresh the executable child bead set for that epic
- Completion evidence:
  - epic has a believable ready path
  - next executable beads exist explicitly

## 4. First Execution Beads

After each rollout bead, list the first concrete child beads.

Each should identify:
- one reviewable outcome
- completion evidence
- canonical truth artifact touched
- matrix rows touched, if validation-related
- blocker/follow-up behavior if the outcome exposes uncovered scope

## 5. Traceability

Record:
- workset -> epic mapping in the workset doc
- bead -> matrix mapping in the active traceability artifact, if applicable

## 6. Checks

Before calling the rollout ready:
1. workset has explicit epics
2. each active epic has at least one rollout bead
3. at least one next ready execution bead exists
4. no major execution lane remains only narrative
