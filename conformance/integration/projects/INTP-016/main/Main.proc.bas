Option Explicit

Sub Main()
    Dim a As Long
    Dim b As Long
    a = TestAdd()
    b = TestMul()
End Sub

Function TestAdd() As Long
    Dim c As New Counter
    TestAdd = c.Add(3, 4)
End Function

Function TestMul() As Long
    Dim m As New Adder
    TestMul = m.Multiply(5, 7)
End Function
