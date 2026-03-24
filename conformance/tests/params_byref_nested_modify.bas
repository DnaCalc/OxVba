Sub Main()
    Dim x
    x = 1
    Call Outer(x)
End Sub

Sub Outer(ByRef v)
    v = v + 10
    Call Inner(v)
End Sub

Sub Inner(ByRef v)
    v = v + 100
End Sub
