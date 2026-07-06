Sub Main()
    Dim score As Long
    Dim position As Long
    Dim item
    Dim a1() As String * 1
    Dim a5() As String * 5

    ReDim a1(0 To 2)
    a1(0) = "alpha"
    a1(1) = "B"
    a1(2) = "xy"

    For Each item In a1
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (100 * Len(item))
    Next

    ReDim a5(0 To 2)
    a5(0) = "a"
    a5(1) = "bcdefg"
    a5(2) = "xy"

    position = 0
    For Each item In a5
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (1000 * Len(item))
    Next
End Sub
