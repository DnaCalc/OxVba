Attribute VB_Name = "MainModule"
Public Sub Main()
On Error Resume Next
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.Exists()
End Sub
