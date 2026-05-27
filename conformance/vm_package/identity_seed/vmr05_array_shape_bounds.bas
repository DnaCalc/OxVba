Option Base 1

Sub Main()
    Dim fixedL As Long
    Dim fixedU As Long
    Dim explicitL As Long
    Dim explicitU As Long
    Dim dynL As Long
    Dim dynU As Long
    Dim total As Long

    Call ArrayWorker(fixedL, fixedU, explicitL, explicitU, dynL, dynU, total)
End Sub

Sub ArrayWorker(ByRef fixedL As Long, ByRef fixedU As Long, ByRef explicitL As Long, ByRef explicitU As Long, ByRef dynL As Long, ByRef dynU As Long, ByRef total As Long)
    Dim fixed(3) As Long
    Dim explicit(0 To 2) As Long
    Dim dyn() As Long

    fixed(1) = 5
    fixed(3) = 7
    explicit(0) = 11
    explicit(2) = 13
    ReDim dyn(2 To 4)
    dyn(2) = 17
    dyn(4) = 19

    fixedL = LBound(fixed)
    fixedU = UBound(fixed)
    explicitL = LBound(explicit)
    explicitU = UBound(explicit)
    dynL = LBound(dyn)
    dynU = UBound(dyn)
    total = fixed(1) + fixed(3) + explicit(0) + explicit(2) + dyn(2) + dyn(4)
End Sub
