Sub Main()
    Dim score As Long
    Dim position As Long
    Dim item
    Dim a() As String * 3

    ReDim a(0 To 2)
    a(0) = "a"
    a(1) = "abcd"
    a(2) = "xy"

    For Each item In a
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (1000 * Len(item))
    Next
End Sub
