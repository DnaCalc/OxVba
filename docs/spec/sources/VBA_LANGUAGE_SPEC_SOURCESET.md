# VBA Language Spec Source Set

Status: active source-map  
Last updated: 2026-03-01

## Purpose

Local manifest of authoritative language-semantics sources used for OxVba language conformance work.

This file is the local path to use when referencing the language spec source set in plans/worksets/evidence.

## Primary Canonical Sources

1. MS-VBAL root (VBA language specification):
- https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74

2. MS-VBA language reference family (Microsoft VBA docs):
- https://learn.microsoft.com/en-us/office/vba/language/reference/

3. Relevant type/interoperation semantics where language rules rely on automation representation:
- MS-OAUT root: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/

## Project Integration Anchors

- `MACH1000_PLAN.md` (sections: references + MS-VBAL notes).
- `docs/evidence/language/COVERAGE_INDEX.csv` (implementation status by language construct).
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` (oracle topics for uncertain language semantics).

## Local-Only Notes

- No full offline mirror is currently maintained in-repo.
- If a section is operationally critical and unstable, capture a local excerpt hash + section URL in the associated workset/evidence artifact.
