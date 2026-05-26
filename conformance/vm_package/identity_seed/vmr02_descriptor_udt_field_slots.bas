Type Point
X As Long
Y As Long
Caption As String
End Type

Sub Main()
Dim p As Point
Dim q As Point
Dim total As Long
p.X = 7
p.Y = 9
p.Caption = "pt"
q = p
q.X = q.X + 1
total = q.X + q.Y
End Sub
