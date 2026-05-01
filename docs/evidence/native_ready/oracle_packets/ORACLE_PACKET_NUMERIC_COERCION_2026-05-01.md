# Oracle Packet: Numeric / Coercion Selected Rows

Date: 2026-05-01
Matrix row: `NR-ORACLE-001`
Script: `oracle_numeric_coercion_001.bas`

## Purpose

Provide a small Office/VBA capture packet for selected numeric and coercion rows
that are useful before native specialization.

## Capture procedure

1. Open Excel/VBA or another VBA host.
2. Import `oracle_numeric_coercion_001.bas`.
3. Run `OracleNumericCoercion001`.
4. Copy Immediate Window output to a result artifact named
   `oracle_numeric_coercion_001_<host>_<date>.txt`.
5. Record host version, bitness, locale, and any Trust Center/security settings
   needed to run the packet.

## Output schema

CSV-like text:

```text
case_id,result
NR-NUM-ROUND,<value>
NR-NUM-INTDIV,<value>
NR-NUM-MOD,<value>
NR-NUM-POW,<value>
NR-COERCE-EMPTY-ADD,<value>
NR-COERCE-BOOL-ADD,<value>
NR-COERCE-NULL-EQ,<value>
```

If the host raises before completing, record:

```text
ERROR,<Err.Number>,<Err.Description>
```

## CI status

This packet is not required in headless CI. If Office/VBA automation is not
available, matrix row `NR-ORACLE-001` remains `deferred` with this skip
rationale: host-backed Office/VBA capture requires an installed interactive VBA
host.
