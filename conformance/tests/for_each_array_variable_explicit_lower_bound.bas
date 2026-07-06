Sub Main()
    Dim x
    Dim v
    Dim a(2 To 4)
    a(2) = 5
    a(3) = 6
    a(4) = 7
    x = 0
    For Each v In a
        x = x + v
    Next
End Sub
