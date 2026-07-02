Sub Main()
    Dim y
    Probe y
End Sub

Private Sub Probe(ByRef y)
    Dim a
    a = Array(1, 2, 3)
    y = Join(a, 0)
End Sub
