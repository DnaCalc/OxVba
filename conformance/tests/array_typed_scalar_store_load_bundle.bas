Sub Main()
Dim gotBool As Boolean
Dim gotByte As Byte
Dim gotInteger As Integer
Dim gotLong As Long
Dim gotLongLong As LongLong
Dim gotSingle As Single
Dim gotDouble As Double
Dim gotCurrency As Currency
Dim gotDate As Date
Dim gotString As String
Dim ab(0) As Boolean
Dim abyte(0) As Byte
Dim ai(0) As Integer
Dim al(0) As Long
Dim all(0) As LongLong
Dim asg(0) As Single
Dim adbl(0) As Double
Dim acur(0) As Currency
Dim adate(0) As Date
Dim astr(0) As String
ab(0) = True
abyte(0) = CByte(7)
ai(0) = 44%
al(0) = 42&
all(0) = 5000000012^
asg(0) = 1.25!
adbl(0) = 2.5#
acur(0) = 12.3456@
adate(0) = CDate(36527#)
astr(0) = "alpha"
gotBool = ab(0)
gotByte = abyte(0)
gotInteger = ai(0)
gotLong = al(0)
gotLongLong = all(0)
gotSingle = asg(0)
gotDouble = adbl(0)
gotCurrency = acur(0)
gotDate = adate(0)
gotString = astr(0)
End Sub
