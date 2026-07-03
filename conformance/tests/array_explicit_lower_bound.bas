Sub Main()
Dim x
    x = ProbeExplicitLowerBound()
End Sub

Private Function ProbeExplicitLowerBound()
Dim a(5 To 7)
a(6) = 11
ProbeExplicitLowerBound = a(6)
End Function
