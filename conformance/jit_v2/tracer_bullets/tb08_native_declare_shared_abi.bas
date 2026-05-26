Private Declare PtrSafe Function lstrlenW Lib "kernel32" (ByVal lpString As LongPtr) As Long
Private Declare PtrSafe Function lstrlenA Lib "kernel32" (ByVal lpString As LongPtr) As Long
Private Declare PtrSafe Function VarR8FromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Double) As Long
Private Declare PtrSafe Sub RtlMoveMemory Lib "kernel32" (ByVal pDest As LongPtr, ByVal pSource As LongPtr, ByVal length As Long)

Private Function NativeBstrLen() As Long
    Dim textValue As String
    textValue = "alpha"
    NativeBstrLen = lstrlenW(StrPtr(textValue))
End Function

Private Function NativeArrayLen() As Long
    Dim buf(0 To 2) As Byte
    buf(0) = 65
    buf(1) = 66
    buf(2) = 0
    NativeArrayLen = lstrlenA(VarPtr(buf(0)))
End Function

Private Function VariantPointerRoundtrip() As Boolean
    Dim v As Variant
    v = Array(1, 2, 3)
    RtlMoveMemory VarPtr(v), VarPtr(v), 0
    VariantPointerRoundtrip = (VarPtr(v) <> 0)
End Function

Sub Main()
    Dim textLen As Long
    Dim numericOut As Double
    Dim status As Long
    Dim arrayLen As Long
    Dim variantOk As Boolean
    Dim result As Long

    textLen = NativeBstrLen()
    status = VarR8FromI4(123, numericOut)
    arrayLen = NativeArrayLen()
    variantOk = VariantPointerRoundtrip()
    result = textLen + arrayLen + status
End Sub
