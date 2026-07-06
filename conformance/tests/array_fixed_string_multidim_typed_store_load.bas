Sub Main()
    Dim got1 As String
    Dim got3 As String
    Dim got5 As String
    Dim fixed3 As String * 3
    Dim fixed5 As String * 5
    Dim a1(1 To 2, 2 To 3) As String * 1
    Dim a3(1 To 2, 2 To 3) As String * 3
    Dim a5(1 To 2, 2 To 3) As String * 5

    a1(1, 2) = "alpha"
    a1(1, 3) = ""
    a1(2, 2) = "Q"
    a3(1, 2) = "a"
    a3(1, 3) = "abcd"
    a3(2, 3) = "xy"
    a5(1, 2) = "a"
    a5(2, 2) = "abcde"
    a5(2, 3) = "abcdef"

    got1 = a1(1, 2)
    got3 = a3(1, 3)
    got5 = a5(2, 3)
    fixed3 = a3(2, 3)
    fixed5 = a1(2, 2)
End Sub
