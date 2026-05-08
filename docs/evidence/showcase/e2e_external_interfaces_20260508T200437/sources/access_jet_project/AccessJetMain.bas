Public Sub Main()
Dim catalog
Dim cn
Dim createResult
Dim insertedAda
Dim insertedGrace
Dim rs
Dim fieldName
Dim fieldScore
Dim nameValue
Dim scoreValue
Set catalog = CreateObject("ADOX.Catalog")
createResult = DispatchInvoke(catalog, "Create", "Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\\Work\\DnaCalc\\OxVba\\docs\\evidence\\showcase\\e2e_external_interfaces_20260508T200437\\artifacts\\ShowcaseJet.accdb")
Set cn = CreateObject("ADODB.Connection")
Call DispatchInvoke(cn, "Open", "Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\\Work\\DnaCalc\\OxVba\\docs\\evidence\\showcase\\e2e_external_interfaces_20260508T200437\\artifacts\\ShowcaseJet.accdb")
Call DispatchInvoke(cn, "Execute", "CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)")
insertedAda = DispatchInvoke(cn, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)")
insertedGrace = DispatchInvoke(cn, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)")
rs = DispatchInvoke(cn, "Execute", "SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2")
fieldName = DispatchInvoke(rs, "Fields", "Name")
fieldScore = DispatchInvoke(rs, "Fields", "Score")
nameValue = DispatchInvoke(fieldName, "Value")
scoreValue = DispatchInvoke(fieldScore, "Value")
End Sub