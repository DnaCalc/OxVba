Sub Main()
Dim score As Long
Dim d1 As Long
Dim d2 As Integer
Dim ab(1 To 2, 4 To 6) As Boolean
Dim abyte(1 To 2, 4 To 6) As Byte
Dim ai(1 To 2, 4 To 6) As Integer
Dim al(1 To 2, 4 To 6) As Long
Dim all(1 To 2, 4 To 6) As LongLong
Dim asg(1 To 2, 4 To 6) As Single
Dim adbl(1 To 2, 4 To 6) As Double
Dim acur(1 To 2, 4 To 6) As Currency
Dim adate(1 To 2, 4 To 6) As Date
Dim astr(1 To 2, 4 To 6) As String
d1 = 1
d2 = 2
score = score + LBound(ab, d1) + UBound(ab, d1) * 10 + LBound(ab, d2) * 100 + UBound(ab, d2) * 1000
score = score + LBound(abyte, d1) + UBound(abyte, d1) * 10 + LBound(abyte, d2) * 100 + UBound(abyte, d2) * 1000
score = score + LBound(ai, d1) + UBound(ai, d1) * 10 + LBound(ai, d2) * 100 + UBound(ai, d2) * 1000
score = score + LBound(al, d1) + UBound(al, d1) * 10 + LBound(al, d2) * 100 + UBound(al, d2) * 1000
score = score + LBound(all, d1) + UBound(all, d1) * 10 + LBound(all, d2) * 100 + UBound(all, d2) * 1000
score = score + LBound(asg, d1) + UBound(asg, d1) * 10 + LBound(asg, d2) * 100 + UBound(asg, d2) * 1000
score = score + LBound(adbl, d1) + UBound(adbl, d1) * 10 + LBound(adbl, d2) * 100 + UBound(adbl, d2) * 1000
score = score + LBound(acur, d1) + UBound(acur, d1) * 10 + LBound(acur, d2) * 100 + UBound(acur, d2) * 1000
score = score + LBound(adate, d1) + UBound(adate, d1) * 10 + LBound(adate, d2) * 100 + UBound(adate, d2) * 1000
score = score + LBound(astr, d1) + UBound(astr, d1) * 10 + LBound(astr, d2) * 100 + UBound(astr, d2) * 1000
End Sub
