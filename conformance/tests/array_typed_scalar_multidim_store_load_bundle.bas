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
Dim ab(1 To 2, 1 To 3) As Boolean
Dim abyte(1 To 2, 1 To 3) As Byte
Dim ai(1 To 2, 1 To 3) As Integer
Dim al(1 To 2, 1 To 3) As Long
Dim all(1 To 2, 1 To 3) As LongLong
Dim asg(1 To 2, 1 To 3) As Single
Dim adbl(1 To 2, 1 To 3) As Double
Dim acur(1 To 2, 1 To 3) As Currency
Dim adate(1 To 2, 1 To 3) As Date
Dim astr(1 To 2, 1 To 3) As String
ab(2, 3) = True
abyte(2, 3) = CByte(7)
ai(2, 3) = 44%
al(2, 3) = 42&
all(2, 3) = 5000000012^
asg(2, 3) = 1.25!
adbl(2, 3) = 2.5#
acur(2, 3) = 12.3456@
adate(2, 3) = CDate(36527#)
astr(2, 3) = "alpha"
gotBool = ab(2, 3)
gotByte = abyte(2, 3)
gotInteger = ai(2, 3)
gotLong = al(2, 3)
gotLongLong = all(2, 3)
gotSingle = asg(2, 3)
gotDouble = adbl(2, 3)
gotCurrency = acur(2, 3)
gotDate = adate(2, 3)
gotString = astr(2, 3)
End Sub
