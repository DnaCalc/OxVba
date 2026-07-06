Sub Main()
    Dim observed
    Dim item
    Dim a() As Date
    ReDim a(1)
    a(0) = CDate(36533#)
    a(1) = CDate(36534#)
    observed = 99
    For Each item In a
    Next
    observed = item
End Sub
