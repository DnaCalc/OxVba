Public Function JitExportedAdd(ByVal lhs As Long, ByRef rhs As Variant) As Variant
    rhs = rhs + 1
    JitExportedAdd = lhs + rhs
End Function

Sub Main()
    Dim x
    Dim y

    x = 5
    y = JitExportedAdd(7, x)
End Sub
