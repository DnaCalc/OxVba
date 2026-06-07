Attribute VB_Name = "Module1"
Option Explicit

Public Sub Main()
    Dim app As Object
    Set app = CreateObject("Scripting.Dictionary")
    app.Add "answer", 42
    Debug.Print app.Item("answer")
End Sub
