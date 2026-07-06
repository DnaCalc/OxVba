Sub Main()
    Dim got1 As String
    Dim got3 As String
    Dim got5 As String
    Dim fixed3 As String * 3
    Dim fixed5 As String * 5
    Dim a1() As String * 1
    Dim a3() As String * 3
    Dim a5() As String * 5

    ReDim a1(0 To 2)
    ReDim a3(0 To 2)
    ReDim a5(0 To 2)

    a1(0) = "alpha"
    a1(1) = ""
    a1(2) = "Z"
    a3(0) = "a"
    a3(1) = "abcd"
    a3(2) = "xy"
    a5(0) = "a"
    a5(1) = "abcde"
    a5(2) = "abcdef"

    got1 = a1(0)
    got3 = a3(1)
    got5 = a5(2)
    fixed3 = a3(2)
    fixed5 = a1(2)
End Sub
