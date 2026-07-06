Sub Main()
    Dim score As Long
    Dim position As Long
    Dim item
    Dim a(1 To 2, 1 To 2, 1 To 2) As String * 3

    a(1, 1, 1) = "a"
    a(1, 1, 2) = "bcde"
    a(1, 2, 1) = "xy"
    a(1, 2, 2) = "z"
    a(2, 1, 1) = "mno"
    a(2, 1, 2) = "pqrs"
    a(2, 2, 1) = "uv"
    a(2, 2, 2) = "w"

    For Each item In a
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (1000 * Len(item))
    Next
End Sub
