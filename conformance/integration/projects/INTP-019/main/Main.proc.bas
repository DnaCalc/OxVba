Option Explicit

Sub Main()
    Dim a As Long
    Dim b As Long
    Dim c As Long
    Dim d As Long
    a = LibA.Util.Tag()
    b = LibB.Util.Tag()
    c = LibC.Calc.Triple(4)
    d = Tag()
End Sub

Public Function Tag() As Long
    Tag = 99
End Function
