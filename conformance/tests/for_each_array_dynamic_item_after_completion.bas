Sub Main()
    Dim observed
    Dim item
    Dim a()
    ReDim a(1)
    a(0) = 7
    a(1) = 8
    observed = 99
    For Each item In a
    Next
    observed = item
End Sub
