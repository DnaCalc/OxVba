Sub Main()
    Dim x
    Dim count
    x = 1
    count = 0
    While x < 100
        Call DoubleIt(x)
        count = count + 1
    Wend
End Sub

Sub DoubleIt(ByRef v)
    v = v * 2
End Sub
