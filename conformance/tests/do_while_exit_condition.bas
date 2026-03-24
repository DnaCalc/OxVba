Sub Main()
    Dim x
    Dim count
    x = 1
    count = 0
    Do While x < 100
        x = x * 2
        count = count + 1
        If x > 50 Then Exit Do
    Loop
End Sub
