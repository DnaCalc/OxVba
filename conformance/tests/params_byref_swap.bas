Sub Main()
    Dim a
    Dim b
    a = 5
    b = 10
    Call DoSwap(a, b)
End Sub

Sub DoSwap(ByRef x, ByRef y)
    Dim tmp
    tmp = x
    x = y
    y = tmp
End Sub
