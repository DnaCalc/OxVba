Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet
    Dim found

    app = CreateObject("Excel.Application")
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheet = DispatchInvoke(book, "Worksheets", 1)
    found = sheet(What:="needle", LookIn:=-4163, LookAt:=1)
    DispatchInvoke(book, "Close", False)
    DispatchInvoke(app, "Quit")
End Sub
