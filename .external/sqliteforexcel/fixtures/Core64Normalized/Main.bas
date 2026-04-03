Attribute VB_Name = "Main"
Option Explicit

Public Sub Main()
    Dim initResult As Long

    initResult = SQLite3Initialize(ThisWorkbook.Path & "\x64")
    Print initResult
    Print SQLite3LibVersion()
    Call SQLite3Free
End Sub
