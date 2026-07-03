Sub Main()
Dim remainingCount As Long
remainingCount = CountAfterRemove()
End Sub

Private Function CountAfterRemove() As Long
Dim c As New Collection
c.Add 9
c.Add 10
c.Remove 1
CountAfterRemove = c.Count
End Function
