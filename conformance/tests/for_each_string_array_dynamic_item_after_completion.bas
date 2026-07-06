Sub Main()
    Dim observed
    Dim item
    Dim a() As String
    ReDim a(1)
    a(0) = "first"
    a(1) = "second"
    observed = 99
    For Each item In a
    Next
    observed = item
End Sub
