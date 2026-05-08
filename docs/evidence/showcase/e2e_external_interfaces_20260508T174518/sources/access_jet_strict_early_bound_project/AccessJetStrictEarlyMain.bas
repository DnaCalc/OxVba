Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs As ADODB.Recordset
Dim fieldName As ADODB.Field
Dim fieldScore As ADODB.Field
Dim nameValue
Dim scoreValue
Dim bangNameValue
Dim bangScoreValue
Call catalog.Create("Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\Work\DnaCalc\OxVba\docs\evidence\showcase\e2e_external_interfaces_20260508T174518\artifacts\ShowcaseJetStrictEarlyBound.accdb")
Call cn.Open("Provider=Microsoft.ACE.OLEDB.12.0;Data Source=C:\Work\DnaCalc\OxVba\docs\evidence\showcase\e2e_external_interfaces_20260508T174518\artifacts\ShowcaseJetStrictEarlyBound.accdb", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
Set rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
nameValue = fieldName.Value
scoreValue = fieldScore.Value
bangNameValue = rs!Name
bangScoreValue = rs!Score
End Sub