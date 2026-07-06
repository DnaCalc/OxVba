Sub Main()
    Dim score As Long
    Dim position As Long
    Dim item
    Dim a1() As String * 1
    Dim a5() As String * 5

    ReDim a1(1 To 2, 1 To 2, 1 To 2)
    a1(1, 1, 1) = "alpha"
    a1(1, 1, 2) = "B"
    a1(1, 2, 1) = "xy"
    a1(1, 2, 2) = "mno"
    a1(2, 1, 1) = "q"
    a1(2, 1, 2) = "rs"
    a1(2, 2, 1) = "uvwx"
    a1(2, 2, 2) = "Z"

    For Each item In a1
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (100 * Len(item))
    Next

    ReDim a5(1 To 2, 1 To 2, 1 To 2)
    a5(1, 1, 1) = "a"
    a5(1, 1, 2) = "bcdefg"
    a5(1, 2, 1) = "xy"
    a5(1, 2, 2) = "qrstuv"
    a5(2, 1, 1) = "m"
    a5(2, 1, 2) = "nopqrs"
    a5(2, 2, 1) = "uv"
    a5(2, 2, 2) = "wxyzab"

    position = 0
    For Each item In a5
        position = position + 1
        score = score + (position * AscW(item))
        score = score + (1000 * Len(item))
    Next
End Sub
