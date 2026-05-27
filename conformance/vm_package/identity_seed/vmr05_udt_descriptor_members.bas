Type Point
X As Long
Y As Long
End Type

Type Record
Name As String * 5
Scores(1 To 2) As Long
Inner As Point
End Type

Sub Main()
Dim r As Record
Dim total As Long
r.Name = "abc"
r.Scores_1 = 4
r.Scores_2 = 5
r.Inner_X = 3
r.Inner_Y = 6
total = r.Scores_1 + r.Scores_2 + r.Inner_X + r.Inner_Y
End Sub
