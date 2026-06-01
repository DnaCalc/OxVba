/// Every kind of token and composite node in the VBA syntax tree.
///
/// Token kinds are leaf nodes (produced by the lexer).
/// Node kinds are interior nodes (produced by the parser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // ── Trivia ───────────────────────────────────────────────
    Whitespace,
    Newline,
    Comment,
    LineContinuation,

    // ── Literals ─────────────────────────────────────────────
    IntLiteral,
    FloatLiteral,
    HexLiteral,
    OctLiteral,
    StringLiteral,
    DateLiteral,

    // ── Identifiers ──────────────────────────────────────────
    Ident,
    BracketedIdent, // [Sheet1]
    TypeSuffix,     // %, &, !, #, @, $

    // ── Keywords ─────────────────────────────────────────────
    KwSub,
    KwFunction,
    KwEnd,
    KwIf,
    KwThen,
    KwElse,
    KwElseIf,
    KwFor,
    KwTo,
    KwStep,
    KwNext,
    KwDo,
    KwLoop,
    KwWhile,
    KwWend,
    KwUntil,
    KwSelect,
    KwCase,
    KwWith,
    KwDim,
    KwConst,
    KwPublic,
    KwPrivate,
    KwStatic,
    KwFriend,
    KwSet,
    KwLet,
    KwAs,
    KwNew,
    KwNothing,
    KwByRef,
    KwByVal,
    KwOptional,
    KwParamArray,
    KwProperty,
    KwGet,
    KwDeclare,
    KwLib,
    KwAlias,
    KwType,
    KwEnum,
    KwEvent,
    KwRaiseEvent,
    KwWithEvents,
    KwImplements,
    KwGoTo,
    KwGoSub,
    KwReturn,
    KwCall,
    KwExit,
    KwOn,
    KwError,
    KwResume,
    KwErase,
    KwReDim,
    KwPreserve,
    KwOption,
    KwExplicit,
    KwBase,
    KwCompare,
    KwAttribute,
    KwMe,
    KwTrue,
    KwFalse,
    KwEmpty,
    KwNull,
    KwNot,
    KwAnd,
    KwOr,
    KwXor,
    KwMod,
    KwLike,
    KwIs,
    KwTypeOf,
    KwImp,
    KwEqv,
    KwDebug,
    KwStop,
    KwEach,
    KwIn,
    KwPrint,
    KwOpen,
    KwClose,
    KwInput,
    KwOutput,
    KwAppend,
    KwBinary,
    KwRandom,
    KwAccess,
    KwRead,
    KwWrite,
    KwLock,
    KwShared,
    KwLine,
    KwName,

    // ── Operators ────────────────────────────────────────────
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    Backslash, // \ (integer division)
    Caret,     // ^
    Ampersand, // &
    Eq,        // =
    Lt,        // <
    Gt,        // >
    LtEq,      // <=
    GtEq,      // >=
    LtGt,      // <>
    ColonEq,   // :=

    // ── Punctuation ──────────────────────────────────────────
    LParen,    // (
    RParen,    // )
    Comma,     // ,
    Dot,       // .
    Bang,      // !
    Colon,     // :
    Semicolon, // ;
    Hash,      // #

    // ── Composite nodes (parser-produced) ────────────────────
    SourceFile,
    SubDecl,
    FunctionDecl,
    PropertyDecl,
    DeclareStmt,
    DimStmt,
    ConstStmt,
    TypeBlock,
    EnumBlock,
    IfStmt,
    ElseIfClause,
    ElseClause,
    ForStmt,
    ForEachStmt,
    DoStmt,
    WhileStmt,
    SelectStmt,
    CaseClause,
    DefaultCaseClause,
    WithStmt,
    AssignStmt,
    SetStmt,
    LetStmt,
    CallStmt,
    OnErrorStmt,
    ResumeStmt,
    ReDimStmt,
    EraseStmt,
    ExitStmt,
    GoToStmt,
    GoSubStmt,
    ReturnStmt,
    RaiseEventStmt,
    OptionStmt,
    AttributeStmt,
    ImplementsStmt,
    EventDecl,
    ParamList,
    Param,
    TypeRef,
    ArgList,
    Block,
    BinaryExpr,
    UnaryExpr,
    CallExpr,
    MemberExpr,
    IndexExpr,
    NewExpr,
    IdentExpr,
    LiteralExpr,
    ParenExpr,

    // ── Sentinels ────────────────────────────────────────────
    ErrorNode,
    Eof,
}

impl SyntaxKind {
    /// True if this kind represents trivia (whitespace, comments, continuations).
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::Comment
                | SyntaxKind::LineContinuation
        )
    }

    /// True if this kind is a keyword.
    pub fn is_keyword(self) -> bool {
        (self as u16) >= (SyntaxKind::KwSub as u16) && (self as u16) <= (SyntaxKind::KwName as u16)
    }

    /// True if this kind represents a composite (interior) node.
    pub fn is_node(self) -> bool {
        (self as u16) >= (SyntaxKind::SourceFile as u16)
            && (self as u16) <= (SyntaxKind::ParenExpr as u16)
    }

    /// True if this kind represents a token (leaf) — not a composite node.
    pub fn is_token(self) -> bool {
        !self.is_node()
    }
}

/// Map a case-insensitive keyword string to its `SyntaxKind`.
/// Returns `None` if the word is not a VBA keyword.
pub fn keyword_kind(word: &str) -> Option<SyntaxKind> {
    // Caller must provide a lowered string.
    match word {
        "sub" => Some(SyntaxKind::KwSub),
        "function" => Some(SyntaxKind::KwFunction),
        "end" => Some(SyntaxKind::KwEnd),
        "if" => Some(SyntaxKind::KwIf),
        "then" => Some(SyntaxKind::KwThen),
        "else" => Some(SyntaxKind::KwElse),
        "elseif" => Some(SyntaxKind::KwElseIf),
        "for" => Some(SyntaxKind::KwFor),
        "to" => Some(SyntaxKind::KwTo),
        "step" => Some(SyntaxKind::KwStep),
        "next" => Some(SyntaxKind::KwNext),
        "do" => Some(SyntaxKind::KwDo),
        "loop" => Some(SyntaxKind::KwLoop),
        "while" => Some(SyntaxKind::KwWhile),
        "wend" => Some(SyntaxKind::KwWend),
        "until" => Some(SyntaxKind::KwUntil),
        "select" => Some(SyntaxKind::KwSelect),
        "case" => Some(SyntaxKind::KwCase),
        "with" => Some(SyntaxKind::KwWith),
        "dim" => Some(SyntaxKind::KwDim),
        "const" => Some(SyntaxKind::KwConst),
        "public" => Some(SyntaxKind::KwPublic),
        "private" => Some(SyntaxKind::KwPrivate),
        "static" => Some(SyntaxKind::KwStatic),
        "friend" => Some(SyntaxKind::KwFriend),
        "set" => Some(SyntaxKind::KwSet),
        "let" => Some(SyntaxKind::KwLet),
        "as" => Some(SyntaxKind::KwAs),
        "new" => Some(SyntaxKind::KwNew),
        "nothing" => Some(SyntaxKind::KwNothing),
        "byref" => Some(SyntaxKind::KwByRef),
        "byval" => Some(SyntaxKind::KwByVal),
        "optional" => Some(SyntaxKind::KwOptional),
        "paramarray" => Some(SyntaxKind::KwParamArray),
        "property" => Some(SyntaxKind::KwProperty),
        "get" => Some(SyntaxKind::KwGet),
        "declare" => Some(SyntaxKind::KwDeclare),
        "lib" => Some(SyntaxKind::KwLib),
        "alias" => Some(SyntaxKind::KwAlias),
        "type" => Some(SyntaxKind::KwType),
        "enum" => Some(SyntaxKind::KwEnum),
        "event" => Some(SyntaxKind::KwEvent),
        "raiseevent" => Some(SyntaxKind::KwRaiseEvent),
        "withevents" => Some(SyntaxKind::KwWithEvents),
        "implements" => Some(SyntaxKind::KwImplements),
        "goto" => Some(SyntaxKind::KwGoTo),
        "gosub" => Some(SyntaxKind::KwGoSub),
        "return" => Some(SyntaxKind::KwReturn),
        "call" => Some(SyntaxKind::KwCall),
        "exit" => Some(SyntaxKind::KwExit),
        "on" => Some(SyntaxKind::KwOn),
        "error" => Some(SyntaxKind::KwError),
        "resume" => Some(SyntaxKind::KwResume),
        "erase" => Some(SyntaxKind::KwErase),
        "redim" => Some(SyntaxKind::KwReDim),
        "preserve" => Some(SyntaxKind::KwPreserve),
        "option" => Some(SyntaxKind::KwOption),
        "explicit" => Some(SyntaxKind::KwExplicit),
        "base" => Some(SyntaxKind::KwBase),
        "compare" => Some(SyntaxKind::KwCompare),
        "attribute" => Some(SyntaxKind::KwAttribute),
        "me" => Some(SyntaxKind::KwMe),
        "true" => Some(SyntaxKind::KwTrue),
        "false" => Some(SyntaxKind::KwFalse),
        "empty" => Some(SyntaxKind::KwEmpty),
        "null" => Some(SyntaxKind::KwNull),
        "not" => Some(SyntaxKind::KwNot),
        "and" => Some(SyntaxKind::KwAnd),
        "or" => Some(SyntaxKind::KwOr),
        "xor" => Some(SyntaxKind::KwXor),
        "mod" => Some(SyntaxKind::KwMod),
        "like" => Some(SyntaxKind::KwLike),
        "is" => Some(SyntaxKind::KwIs),
        "typeof" => Some(SyntaxKind::KwTypeOf),
        "imp" => Some(SyntaxKind::KwImp),
        "eqv" => Some(SyntaxKind::KwEqv),
        "debug" => Some(SyntaxKind::KwDebug),
        "stop" => Some(SyntaxKind::KwStop),
        "each" => Some(SyntaxKind::KwEach),
        "in" => Some(SyntaxKind::KwIn),
        "print" => Some(SyntaxKind::KwPrint),
        "open" => Some(SyntaxKind::KwOpen),
        "close" => Some(SyntaxKind::KwClose),
        "input" => Some(SyntaxKind::KwInput),
        "output" => Some(SyntaxKind::KwOutput),
        "append" => Some(SyntaxKind::KwAppend),
        "binary" => Some(SyntaxKind::KwBinary),
        "random" => Some(SyntaxKind::KwRandom),
        "access" => Some(SyntaxKind::KwAccess),
        "read" => Some(SyntaxKind::KwRead),
        "write" => Some(SyntaxKind::KwWrite),
        "lock" => Some(SyntaxKind::KwLock),
        "shared" => Some(SyntaxKind::KwShared),
        "line" => Some(SyntaxKind::KwLine),
        "name" => Some(SyntaxKind::KwName),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lookup() {
        assert_eq!(keyword_kind("sub"), Some(SyntaxKind::KwSub));
        assert_eq!(keyword_kind("function"), Some(SyntaxKind::KwFunction));
        assert_eq!(keyword_kind("notakeyword"), None);
    }

    #[test]
    fn kind_classification() {
        assert!(SyntaxKind::Whitespace.is_trivia());
        assert!(SyntaxKind::Comment.is_trivia());
        assert!(!SyntaxKind::Ident.is_trivia());
        assert!(SyntaxKind::KwSub.is_keyword());
        assert!(SyntaxKind::KwName.is_keyword());
        assert!(!SyntaxKind::Ident.is_keyword());
        assert!(SyntaxKind::SourceFile.is_node());
        assert!(!SyntaxKind::Ident.is_node());
        assert!(SyntaxKind::Ident.is_token());
    }
}
