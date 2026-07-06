Sub Main()
Dim score As Long
Dim ab(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Boolean
Dim abyte(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Byte
Dim ai(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Integer
Dim al(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Long
Dim all(1 To 2, 4 To 6, 7 To 9, 10 To 12) As LongLong
Dim asg(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Single
Dim adbl(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Double
Dim acur(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Currency
Dim adate(1 To 2, 4 To 6, 7 To 9, 10 To 12) As Date
Dim astr(1 To 2, 4 To 6, 7 To 9, 10 To 12) As String
score = score + LBound(ab, 1) + UBound(ab, 1) * 10 + LBound(ab, 2) * 100 + UBound(ab, 2) * 1000 + LBound(ab, 3) * 10000 + UBound(ab, 3) * 100000 + LBound(ab, 4) * 1000000 + UBound(ab, 4) * 10000000
score = score + LBound(abyte, 1) + UBound(abyte, 1) * 10 + LBound(abyte, 2) * 100 + UBound(abyte, 2) * 1000 + LBound(abyte, 3) * 10000 + UBound(abyte, 3) * 100000 + LBound(abyte, 4) * 1000000 + UBound(abyte, 4) * 10000000
score = score + LBound(ai, 1) + UBound(ai, 1) * 10 + LBound(ai, 2) * 100 + UBound(ai, 2) * 1000 + LBound(ai, 3) * 10000 + UBound(ai, 3) * 100000 + LBound(ai, 4) * 1000000 + UBound(ai, 4) * 10000000
score = score + LBound(al, 1) + UBound(al, 1) * 10 + LBound(al, 2) * 100 + UBound(al, 2) * 1000 + LBound(al, 3) * 10000 + UBound(al, 3) * 100000 + LBound(al, 4) * 1000000 + UBound(al, 4) * 10000000
score = score + LBound(all, 1) + UBound(all, 1) * 10 + LBound(all, 2) * 100 + UBound(all, 2) * 1000 + LBound(all, 3) * 10000 + UBound(all, 3) * 100000 + LBound(all, 4) * 1000000 + UBound(all, 4) * 10000000
score = score + LBound(asg, 1) + UBound(asg, 1) * 10 + LBound(asg, 2) * 100 + UBound(asg, 2) * 1000 + LBound(asg, 3) * 10000 + UBound(asg, 3) * 100000 + LBound(asg, 4) * 1000000 + UBound(asg, 4) * 10000000
score = score + LBound(adbl, 1) + UBound(adbl, 1) * 10 + LBound(adbl, 2) * 100 + UBound(adbl, 2) * 1000 + LBound(adbl, 3) * 10000 + UBound(adbl, 3) * 100000 + LBound(adbl, 4) * 1000000 + UBound(adbl, 4) * 10000000
score = score + LBound(acur, 1) + UBound(acur, 1) * 10 + LBound(acur, 2) * 100 + UBound(acur, 2) * 1000 + LBound(acur, 3) * 10000 + UBound(acur, 3) * 100000 + LBound(acur, 4) * 1000000 + UBound(acur, 4) * 10000000
score = score + LBound(adate, 1) + UBound(adate, 1) * 10 + LBound(adate, 2) * 100 + UBound(adate, 2) * 1000 + LBound(adate, 3) * 10000 + UBound(adate, 3) * 100000 + LBound(adate, 4) * 1000000 + UBound(adate, 4) * 10000000
score = score + LBound(astr, 1) + UBound(astr, 1) * 10 + LBound(astr, 2) * 100 + UBound(astr, 2) * 1000 + LBound(astr, 3) * 10000 + UBound(astr, 3) * 100000 + LBound(astr, 4) * 1000000 + UBound(astr, 4) * 10000000
End Sub
