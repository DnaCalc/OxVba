Sub Main()
    Dim score As Long
    Dim position As Long
    Dim item
    Dim a1() As String * 1
    Dim a5() As String * 5

    ReDim a1(1 To 2, 1 To 2)
    a1(1, 1) = "alpha"
    a1(1, 2) = "B"
    a1(2, 1) = "xy"
    a1(2, 2) = "mno"

    For Each item In a1
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (100 * Len(item))
    Next

    ReDim a5(1 To 2, 1 To 2)
    a5(1, 1) = "a"
    a5(1, 2) = "bcdefg"
    a5(2, 1) = "xy"
    a5(2, 2) = "qrstuv"

    position = 0
    For Each item In a5
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (1000 * Len(item))
    Next
End Sub
