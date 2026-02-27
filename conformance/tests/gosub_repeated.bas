Sub Main()
Dim x
x = 1
GoSub add_two
GoSub add_two

If Err.Number = -1 Then
add_two:
x = x + 2
Return
End If
End Sub
