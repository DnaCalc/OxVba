Sub Main()
    Dim app
    Dim db
    Dim records
    Dim countValue

    app = CreateObject("Access.Application")
    db = DispatchInvoke(app, "CurrentDb")
    records = DispatchInvoke(db, "OpenRecordset", "SELECT 1 AS One")
    countValue = DispatchInvoke(records, "RecordCount")
    DispatchInvoke(records, "Close")
    DispatchInvoke(app, "Quit")
End Sub

