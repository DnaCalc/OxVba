Sub Main()
    Dim conn
    Dim connectionString
    Dim stateValue

    conn = CreateObject("ADODB.Connection")
    connectionString = "Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\Temp\oxvba-office-com-corpus.accdb;"
    DispatchInvoke(conn, "Open", connectionString)
    stateValue = DispatchInvoke(conn, "State")
    DispatchInvoke(conn, "Close")
End Sub

