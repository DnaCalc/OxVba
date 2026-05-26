Sub Main()
    Dim obj
    Dim countValue
    Dim existsValue
    Dim missingErr

    obj = CreateObject("OxVba.TestDispatch")
    countValue = DispatchInvoke(obj, "Count")
    existsValue = DispatchInvoke(obj, "Exists", 42)

    On Error Resume Next
    Call DispatchInvoke(obj, "RaiseException")
    missingErr = Err.Number
End Sub
