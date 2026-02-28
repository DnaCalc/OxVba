Sub Main()
Dim x
With x.inner
    .Value = 4
    .Value = .Value + 3
    x = .Value
End With
End Sub

' EXPECT: 7,7
