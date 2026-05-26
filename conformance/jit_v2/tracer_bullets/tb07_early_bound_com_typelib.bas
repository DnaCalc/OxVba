Attribute VB_Name = "MainModule"
Public Sub Main()
    Dim obj As New OxVba.TestDispatch
    Dim countValue
    Dim existsValue

    countValue = obj.Count()
    existsValue = obj.Exists(42)
End Sub
