Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.Exists(42)
End Sub
