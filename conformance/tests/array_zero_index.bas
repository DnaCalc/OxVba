Sub Main()
    Dim x
    x = ProbeArrayZeroIndex()
End Sub

Private Function ProbeArrayZeroIndex()
    Dim a(2)
    a(0) = 3
    ProbeArrayZeroIndex = a(0)
End Function
