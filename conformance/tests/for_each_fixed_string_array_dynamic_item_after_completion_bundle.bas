Sub Main()
    Dim score As Long
    Dim item
    Dim a1() As String * 1
    Dim a3() As String * 3
    Dim a5() As String * 5

    ReDim a1(0 To 2)
    a1(0) = "alpha"
    For Each item In a1
    Next
    If IsEmpty(item) Then score = score + 1

    ReDim a3(0 To 2)
    a3(0) = "beta"
    For Each item In a3
    Next
    If IsEmpty(item) Then score = score + 2

    ReDim a5(0 To 2)
    a5(0) = "gamma"
    For Each item In a5
    Next
    If IsEmpty(item) Then score = score + 4
End Sub
