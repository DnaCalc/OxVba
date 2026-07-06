Sub Main()
    Dim observed
    Dim item
    Dim a() As Boolean
    ReDim a(2 To 4)
    a(2) = False
    a(3) = False
    a(4) = True
    For Each item In a
        observed = item
    Next
End Sub
