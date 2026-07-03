Sub Main()
Dim item As Long
Dim defaultItem As Long
item = FirstCollectionItem()
defaultItem = FirstCollectionDefaultItem()
End Sub

Private Function FirstCollectionItem() As Long
Dim c As New Collection
c.Add 9
FirstCollectionItem = c.Item(1)
End Function

Private Function FirstCollectionDefaultItem() As Long
Dim c As New Collection
c.Add 9
FirstCollectionDefaultItem = c(1)
End Function
