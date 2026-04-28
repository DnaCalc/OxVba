Attribute VB_Name = "Application"
Attribute VB_PredeclaredId = True
Attribute VB_Exposed = True

Public Property Get Value() As Object
    Set Value = CreateObject("Excel.Application")
End Property
