Sub Main()
    Dim x
    Dim v
    ProbeForEachArrayVariable x, v
End Sub

Private Sub ProbeForEachArrayVariable(ByRef x, ByRef v)
    Dim a(2)
    a(0) = 4
    a(1) = 5
    a(2) = 6
    For Each v In a
        x = v
    Next
End Sub
