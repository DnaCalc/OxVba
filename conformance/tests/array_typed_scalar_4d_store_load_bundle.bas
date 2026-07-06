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
Dim ab(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Boolean
Dim abyte(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Byte
Dim ai(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Integer
Dim al(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Long
Dim all(1 To 2, 1 To 3, 1 To 4, 1 To 5) As LongLong
Dim asg(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Single
Dim adbl(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Double
Dim acur(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Currency
Dim adate(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Date
Dim astr(1 To 2, 1 To 3, 1 To 4, 1 To 5) As String
ab(2, 3, 4, 5) = True
abyte(2, 3, 4, 5) = CByte(7)
ai(2, 3, 4, 5) = 44%
al(2, 3, 4, 5) = 42&
all(2, 3, 4, 5) = 5000000012^
asg(2, 3, 4, 5) = 1.25!
adbl(2, 3, 4, 5) = 2.5#
acur(2, 3, 4, 5) = 12.3456@
adate(2, 3, 4, 5) = CDate(36527#)
astr(2, 3, 4, 5) = "alpha"
gotBool = ab(2, 3, 4, 5)
gotByte = abyte(2, 3, 4, 5)
gotInteger = ai(2, 3, 4, 5)
gotLong = al(2, 3, 4, 5)
gotLongLong = all(2, 3, 4, 5)
gotSingle = asg(2, 3, 4, 5)
gotDouble = adbl(2, 3, 4, 5)
gotCurrency = acur(2, 3, 4, 5)
gotDate = adate(2, 3, 4, 5)
gotString = astr(2, 3, 4, 5)
End Sub
