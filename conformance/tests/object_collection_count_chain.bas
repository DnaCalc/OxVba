Sub Main()
Dim firstCount As Long
Dim secondCount As Long
firstCount = CountAfterOneAdd()
secondCount = CountAfterTwoAdds()
End Sub

Private Function CountAfterOneAdd() As Long
Dim c As New Collection
c.Add 2
CountAfterOneAdd = c.Count
End Function

Private Function CountAfterTwoAdds() As Long
Dim c As New Collection
c.Add 2
c.Add 3
CountAfterTwoAdds = c.Count
End Function
