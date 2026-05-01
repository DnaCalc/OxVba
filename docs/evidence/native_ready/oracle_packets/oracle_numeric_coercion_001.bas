Attribute VB_Name = "OracleNumericCoercion001"
Option Explicit

' Native-ready oracle packet NR-ORACLE-001.
' Run inside Excel/VBA Immediate-capable host and copy Debug.Print output into
' the packet result artifact.

Public Sub OracleNumericCoercion001()
    On Error GoTo CaptureErr

    Debug.Print "case_id,result"
    Debug.Print "NR-NUM-ROUND," & CStr(Round(19, -1))
    Debug.Print "NR-NUM-INTDIV," & CStr(7 \ 2)
    Debug.Print "NR-NUM-MOD," & CStr(7 Mod 2)
    Debug.Print "NR-NUM-POW," & CStr(2 ^ 10)
    Debug.Print "NR-COERCE-EMPTY-ADD," & CStr(Empty + 5)
    Debug.Print "NR-COERCE-BOOL-ADD," & CStr(CInt(True) + 2)
    Debug.Print "NR-COERCE-NULL-EQ," & CStr(IsNull(Null = 0))
    Exit Sub

CaptureErr:
    Debug.Print "ERROR," & CStr(Err.Number) & "," & Err.Description
End Sub
