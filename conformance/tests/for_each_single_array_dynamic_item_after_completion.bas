Sub Main()
    Dim observed
    Dim item
    Dim a() As Single
    ReDim a(1)
    a(0) = 7.25!
    a(1) = 8.5!
    observed = 99
    For Each item In a
    Next
    observed = item
End Sub
