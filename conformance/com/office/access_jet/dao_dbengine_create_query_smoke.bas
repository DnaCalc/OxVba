Sub Main()
    Dim engine
    Dim db
    Dim records
    Dim nameValue
    Dim scoreValue

    engine = CreateObject("DAO.DBEngine.120")
    db = DispatchInvoke(engine, "CreateDatabase", "C:\Temp\oxvba-office-com-corpus-dao.accdb", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
    DispatchInvoke(db, "Execute", "CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)")
    DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)")
    DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)")
    records = DispatchInvoke(db, "OpenRecordset", "SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2")
    nameValue = DispatchInvoke(DispatchInvoke(records, "Fields", "Name"), "Value")
    scoreValue = DispatchInvoke(DispatchInvoke(records, "Fields", "Score"), "Value")
    DispatchInvoke(records, "Close")
    DispatchInvoke(db, "Close")
End Sub
