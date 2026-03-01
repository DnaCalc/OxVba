# VBA Runtime/Library Spec Source Set

Status: active source-map  
Last updated: 2026-03-01

## Purpose

Local manifest of authoritative runtime/library function sources used for OxVba intrinsic/runtime conformance work.

This file is the local path to use when referencing the library spec source set in plans/worksets/evidence.

## Primary Canonical Sources

1. MS-VBAL function/semantic sections:
- https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74

2. VBA Language Reference (functions):
- https://learn.microsoft.com/en-us/office/vba/language/reference/functions-visual-basic-for-applications

3. VBA Language Reference (statements, constants, objects):
- https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/

4. Automation type/value semantics used by runtime functions:
- MS-OAUT root: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/

## Project Integration Anchors

- `docs/evidence/runtime/LIBRARY_CHECKLIST.csv` (family-level implementation status).
- `docs/evidence/runtime/INTRINSIC_SURFACE.csv` (runtime intrinsic registry surface).
- `docs/evidence/SPEC_CHECKLIST.md` (rolled-up feature/library status).
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` (uncertain semantic topics needing oracle confirmation).

## Local-Only Notes

- No full offline mirror is currently maintained in-repo.
- Use this manifest as the stable local pointer; specific URLs should be pinned in worksets for high-risk features.
