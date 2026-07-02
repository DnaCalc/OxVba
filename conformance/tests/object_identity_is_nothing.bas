Sub Main()
Dim sameNothing As Boolean
Dim isNothing As Boolean
Probe sameNothing, isNothing
End Sub

Private Sub Probe(ByRef sameNothing As Boolean, ByRef isNothing As Boolean)
Dim a As Object
Dim b As Object
sameNothing = a Is b
isNothing = a Is Nothing
End Sub
