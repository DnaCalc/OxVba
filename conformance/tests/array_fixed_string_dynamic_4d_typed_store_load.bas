Sub Main()
    Dim got1 As String
    Dim got3 As String
    Dim got5 As String
    Dim fixed3 As String * 3
    Dim fixed5 As String * 5
    Dim a1() As String * 1
    Dim a3() As String * 3
    Dim a5() As String * 5

    ReDim a1(1 To 2, 2 To 3, 4 To 5, 6 To 7)
    ReDim a3(1 To 2, 2 To 3, 4 To 5, 6 To 7)
    ReDim a5(1 To 2, 2 To 3, 4 To 5, 6 To 7)

    a1(1, 2, 4, 6) = "alpha"
    a1(1, 3, 5, 7) = ""
    a1(2, 2, 4, 6) = "S"
    a3(1, 2, 4, 6) = "a"
    a3(1, 3, 5, 7) = "abcd"
    a3(2, 3, 5, 7) = "xy"
    a5(1, 2, 4, 6) = "a"
    a5(2, 2, 5, 6) = "abcde"
    a5(2, 3, 5, 7) = "abcdef"

    got1 = a1(1, 2, 4, 6)
    got3 = a3(1, 3, 5, 7)
    got5 = a5(2, 3, 5, 7)
    fixed3 = a3(2, 3, 5, 7)
    fixed5 = a1(2, 2, 4, 6)
End Sub
