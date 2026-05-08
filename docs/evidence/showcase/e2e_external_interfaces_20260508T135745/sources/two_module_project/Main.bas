Public Sub Main()
Dim total
Dim doubled
Dim summary
total = MathHelpers.Add(20, 22)
doubled = Scale(total)
summary = MathHelpers.FormatLabel(doubled)
End Sub

Public Function Scale(ByVal value)
Scale = value * 2
End Function