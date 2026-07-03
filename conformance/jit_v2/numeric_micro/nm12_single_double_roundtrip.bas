Public s As Single
Public d As Double
Public same As Boolean

Sub Main()
    s = CSng(1.23456789)
    d = CDbl(s)
    same = (s = CSng(d))
End Sub
