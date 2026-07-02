VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Host"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = True
Option Explicit
Private child As New Counter

Public Function FieldDimOnly() As String
    FieldDimOnly = CStr(Len(Main.Log)) & "|" & Main.Log
End Function

Public Function FieldFirstMember() As String
    child.Bump
    FieldFirstMember = CStr(child.Total) & "|" & Main.Log
End Function

Public Function FieldIsNothing() As String
    FieldIsNothing = CStr(child Is Nothing) & "|" & Main.Log
End Function

Public Function FieldSetNothingBeforeAccess() As String
    Set child = Nothing
    FieldSetNothingBeforeAccess = CStr(Len(Main.Log)) & "|" & Main.Log
End Function

Public Function FieldSetNothingResurrect() As String
    child.Bump
    Set child = Nothing
    FieldSetNothingResurrect = CStr(child.Total) & "|" & Main.Log
End Function

Public Function FieldBumpTotal() As Long
    child.Bump
    FieldBumpTotal = child.Total
End Function
