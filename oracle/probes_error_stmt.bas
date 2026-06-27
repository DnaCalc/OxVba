Attribute VB_Name = "OracleProbesErrorStmt"
' The legacy `Error <n>` statement. MS-VBAL §2841: it behaves as if Err.Raise(number).
' These probes confirm it routes through the same Err.Raise machinery (incl. §9071
' omitted-arg inheritance), so the runtime can reuse ErrorOp::Raise.

' ---- Error <n> is catchable and populates Err like Err.Raise n ----
Function PROBE_error_statement() As String
    On Error GoTo H
    Error 5
    PROBE_error_statement = "no-fault"
    Exit Function
H:
    PROBE_error_statement = "handler;errnum=" & Err.Number & ";desc=" & Err.Description
End Function

' ---- Error <n> with an un-cleared Err inherits Source/Description (§9071) ----
Function PROBE_error_statement_inherit() As String
    On Error Resume Next
    Err.Raise 5, "Src1", "Desc1"       ' Err = {5, Src1, Desc1}
    Error 6                            ' Error stmt = Err.Raise 6; omit -> inherit?
    PROBE_error_statement_inherit = "n=" & Err.Number & ";src=" & Err.Source & ";desc=" & Err.Description
End Function

' ---- Error <n> from a clean Err uses the derived description ----
Function PROBE_error_statement_clean() As String
    On Error Resume Next
    Error 11
    PROBE_error_statement_clean = "n=" & Err.Number & ";src=[" & Err.Source & "];desc=" & Err.Description
End Function
