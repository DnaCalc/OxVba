Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet
    Dim cell

    app = CreateObject("Excel.Application")
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheet = DispatchInvoke(book, "Worksheets", 1)
    cell = DispatchInvoke(sheet, "Range", "A1")
    book.Close False
    app.Quit
End Sub
