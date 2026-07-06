Sub Main()
Dim score As Long
Dim ab() As Boolean
Dim abyte() As Byte
Dim ai() As Integer
Dim al() As Long
Dim all() As LongLong
Dim asg() As Single
Dim adbl() As Double
Dim acur() As Currency
Dim adate() As Date
Dim astr() As String
ReDim ab(1 To 2, 4 To 6)
ReDim abyte(1 To 2, 4 To 6)
ReDim ai(1 To 2, 4 To 6)
ReDim al(1 To 2, 4 To 6)
ReDim all(1 To 2, 4 To 6)
ReDim asg(1 To 2, 4 To 6)
ReDim adbl(1 To 2, 4 To 6)
ReDim acur(1 To 2, 4 To 6)
ReDim adate(1 To 2, 4 To 6)
ReDim astr(1 To 2, 4 To 6)
score = score + LBound(ab, 1) + UBound(ab, 1) * 10 + LBound(ab, 2) * 100 + UBound(ab, 2) * 1000
score = score + LBound(abyte, 1) + UBound(abyte, 1) * 10 + LBound(abyte, 2) * 100 + UBound(abyte, 2) * 1000
score = score + LBound(ai, 1) + UBound(ai, 1) * 10 + LBound(ai, 2) * 100 + UBound(ai, 2) * 1000
score = score + LBound(al, 1) + UBound(al, 1) * 10 + LBound(al, 2) * 100 + UBound(al, 2) * 1000
score = score + LBound(all, 1) + UBound(all, 1) * 10 + LBound(all, 2) * 100 + UBound(all, 2) * 1000
score = score + LBound(asg, 1) + UBound(asg, 1) * 10 + LBound(asg, 2) * 100 + UBound(asg, 2) * 1000
score = score + LBound(adbl, 1) + UBound(adbl, 1) * 10 + LBound(adbl, 2) * 100 + UBound(adbl, 2) * 1000
score = score + LBound(acur, 1) + UBound(acur, 1) * 10 + LBound(acur, 2) * 100 + UBound(acur, 2) * 1000
score = score + LBound(adate, 1) + UBound(adate, 1) * 10 + LBound(adate, 2) * 100 + UBound(adate, 2) * 1000
score = score + LBound(astr, 1) + UBound(astr, 1) * 10 + LBound(astr, 2) * 100 + UBound(astr, 2) * 1000
End Sub
