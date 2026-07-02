Sub Main()
Dim same As Boolean
Dim different As Boolean
Probe same, different
End Sub

Private Sub Probe(ByRef same As Boolean, ByRef different As Boolean)
Dim a As New Collection
Dim b As New Collection
same = a Is a
different = a Is b
End Sub
