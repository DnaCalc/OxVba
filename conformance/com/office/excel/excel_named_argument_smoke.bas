Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet
    Dim sheets
    Dim added

    app = CreateObject("Excel.Application")
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheets = DispatchInvoke(book, "Worksheets")
    sheet = DispatchInvoke(sheets, "Item", 1)
    added = DispatchInvoke(sheets, "Add", After:=sheet)
    DispatchInvoke(book, "Close", False)
    DispatchInvoke(app, "Quit")
End Sub
