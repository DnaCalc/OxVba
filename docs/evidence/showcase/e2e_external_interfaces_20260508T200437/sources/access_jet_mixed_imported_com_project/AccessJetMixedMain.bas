Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs
Dim fieldName
Dim fieldScore
Dim nameValue
Dim scoreValue
Call DispatchInvoke(catalog, "Create", "Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\\Work\\DnaCalc\\OxVba\\docs\\evidence\\showcase\\e2e_external_interfaces_20260508T200437\\artifacts\\ShowcaseJetMixedImported.accdb")
Call cn.Open("Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\\Work\\DnaCalc\\OxVba\\docs\\evidence\\showcase\\e2e_external_interfaces_20260508T200437\\artifacts\\ShowcaseJetMixedImported.accdb", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
fieldName = DispatchInvoke(rs, "Fields", "Name")
fieldScore = DispatchInvoke(rs, "Fields", "Score")
nameValue = DispatchInvoke(fieldName, "Value")
scoreValue = DispatchInvoke(fieldScore, "Value")
End Sub