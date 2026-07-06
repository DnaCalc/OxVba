Sub Main()
    Dim observed
    Dim item
    Dim a() As Boolean
    ReDim a(2)
    a(0) = False
    a(1) = False
    a(2) = True
    observed = 99
    For Each item In a
    Next
    observed = item
End Sub
