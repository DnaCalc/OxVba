# VBA Grammar V1

Date: 2026-06-01
Status: draft scaffold for frontend v2
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
Bead: `bd-aprs.2.1`

## Purpose

This document is the in-repo grammar anchor for OxVba's frontend v2 work. It is a clean-room
grammar scaffold derived from implementation knowledge, existing OxVba fixtures, and public
authority references. It is not a copied extract of any third-party grammar.

The grammar exists to drive:

- parser implementation and recovery shape;
- grammar-production coverage rows;
- fixture classification for syntax-only, binder, execution, and host-sensitive lanes;
- explicit residuals where OxVba intentionally stages support.

## Authority And Provenance

Primary authority:

- Microsoft Open Specifications, `[MS-VBAL]: VBA Language Specification`
  (`https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74`).

Existing repo evidence:

- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.md`
- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`
- `docs/spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`
- `docs/spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`
- `docs/spec/VBA_TYPE_SYSTEM_V1.md`
- `conformance/tests_manifest.csv`
- `conformance/integration/catalog.psv`

Clean-room rule:

- This file names grammar categories and OxVba target productions in project-owned wording.
- Do not paste normative MS-VBAL grammar text into this repository.
- Rubberduck or other grammars may be used as quirk checklists in later beads, but not copied into
  this grammar artifact.

## Dialect Target

Target dialect for frontend v2:

- VBA 7.x as hosted by modern Windows Office, with Excel as the primary executable oracle.
- Existing OxVba executable language subset remains a regression baseline, not the authority.
- Host/COM/project semantics are in scope when needed for compile-time binding, but userform/forms
  runtime behavior remains outside this grammar scaffold unless later worksets expand it.

Known dialect notes:

- Source is case-insensitive for keywords and identifiers, while preserving original spelling and
  trivia in the CST.
- Physical lines and logical lines differ because line continuations join lines.
- `:` separates statements inside a logical line.
- Comments (`'` and `Rem`) are trivia for binding/lowering but must be preserved losslessly.
- Type characters (`%`, `&`, `^`, `!`, `#`, `@`, `$`) can participate in declaration and
  expression lexing and must be resolved contextually.
- Bracketed identifiers preserve text and allow otherwise special names.
- Date literals, string literals, numeric base prefixes, and numeric suffixes are lexical forms, but
  their semantic value is resolved after tokenization.

## Notation

This scaffold uses EBNF-style project notation:

- `A = B C` means sequence.
- `A | B` means alternative.
- `A?` means optional.
- `A*` means zero or more.
- `A+` means one or more.
- quoted words are keywords or punctuation in source text, compared case-insensitively for
  keywords.
- ALL_CAPS names are token classes.
- lower_snake_case names are grammar productions.

## Top-Level Grammar Scaffold

```ebnf
source_file        = file_preamble? module_body EOF ;
file_preamble      = (attribute_line | option_line | blank_line | comment_line)* ;
module_body        = module_item* ;
module_item        = option_line
                   | attribute_line
                   | declaration
                   | procedure_decl
                   | property_decl
                   | event_decl
                   | implements_decl
                   | statement_line
                   | blank_line
                   | comment_line ;

option_line        = "Option" option_kind option_value? NEWLINE? ;
option_kind        = "Base" | "Compare" | "Explicit" | "Private" ;
option_value       = expression | identifier ;
attribute_line     = "Attribute" qualified_identifier "=" expression NEWLINE? ;
statement_line     = logical_line ;
logical_line       = line_prefix? statement_list? line_comment? NEWLINE? ;
statement_list     = statement (":" statement)* ;
line_comment       = COMMENT ;
line_prefix        = LABEL ":" | LINE_NUMBER ;
blank_line         = trivia* NEWLINE ;
comment_line       = trivia* COMMENT NEWLINE? ;
trivia             = WHITESPACE | LINE_CONTINUATION ;
```

## Declarations

```ebnf
declaration        = const_decl
                   | dim_decl
                   | variable_decl_stmt
                   | static_decl
                   | declare_decl
                   | enum_decl
                   | type_decl ;

const_decl         = visibility? "Const" const_item ("," const_item)* ;
const_item         = identifier type_ascription? "=" expression ;

dim_decl           = "Dim" variable_decl_list ;
variable_decl_stmt = ("Dim" | visibility) variable_decl_list ;
static_decl        = "Static" variable_decl_list ;

variable_decl_list = variable_decl ("," variable_decl)* ;
variable_decl      = identifier array_rank? type_ascription? ;
array_rank         = "(" bound_list? ")" ;
bound_list         = bound ("," bound)* ;
bound              = expression ("To" expression)? ;
type_ascription    = "As" new_modifier? type_name ;
new_modifier       = "New" ;

enum_decl          = visibility? "Enum" identifier enum_member* "End" "Enum" ;
enum_member        = identifier ("=" expression)? ;

type_decl          = visibility? "Type" identifier field_decl* "End" "Type" ;
field_decl         = identifier array_rank? type_ascription? ;

declare_decl       = visibility? "Declare" ("PtrSafe")? ("Sub" | "Function")
                     identifier lib_clause alias_clause? parameter_list? type_ascription? ;
lib_clause         = "Lib" STRING_LITERAL ;
alias_clause       = "Alias" STRING_LITERAL ;

event_decl         = visibility? "Event" identifier parameter_list? ;
implements_decl    = "Implements" type_name ;
```

## Procedures And Properties

```ebnf
procedure_decl     = sub_decl | function_decl ;
sub_decl           = visibility? "Sub" identifier parameter_list? statement_block "End" "Sub" ;
function_decl      = visibility? "Function" identifier parameter_list? type_ascription?
                     statement_block "End" "Function" ;

property_decl      = property_get | property_let | property_set ;
property_get       = visibility? "Property" "Get" identifier parameter_list? type_ascription?
                     statement_block "End" "Property" ;
property_let       = visibility? "Property" "Let" identifier parameter_list
                     statement_block "End" "Property" ;
property_set       = visibility? "Property" "Set" identifier parameter_list
                     statement_block "End" "Property" ;

parameter_list     = "(" parameter_decl_list? ")" ;
parameter_decl_list = parameter_decl ("," parameter_decl)* ;
parameter_decl     = parameter_modifier* "ParamArray"? identifier array_rank?
                     type_ascription? default_value? ;
parameter_modifier = "Optional" | "ByVal" | "ByRef" ;
default_value      = "=" expression ;

statement_block    = logical_line* ;
visibility         = "Public" | "Private" | "Friend" | "Global" ;
```

## Statements

```ebnf
statement          = assignment_stmt
                   | call_stmt
                   | if_stmt
                   | select_stmt
                   | for_stmt
                   | for_each_stmt
                   | do_loop_stmt
                   | while_wend_stmt
                   | with_stmt
                   | on_error_stmt
                   | resume_stmt
                   | goto_stmt
                   | gosub_stmt
                   | return_stmt
                   | exit_stmt
                   | erase_stmt
                   | redim_stmt
                   | raise_event_stmt
                   | local_decl_stmt
                   | empty_stmt ;

assignment_stmt    = ("Set" | "Let")? lvalue "=" expression ;
call_stmt          = "Call"? callable_expr argument_syntax? ;
if_stmt            = inline_if_stmt | block_if_stmt ;
inline_if_stmt     = "If" expression "Then" statement_list ("Else" statement_list)? ;
block_if_stmt      = "If" expression "Then" logical_line*
                     elseif_clause* else_clause? "End" "If" ;
elseif_clause      = "ElseIf" expression "Then" logical_line* ;
else_clause        = "Else" logical_line* ;

select_stmt        = "Select" "Case" expression logical_line*
                     case_clause* "End" "Select" ;
case_clause        = "Case" case_selector_list logical_line* ;
case_selector_list = "Else" | case_selector ("," case_selector)* ;
case_selector      = expression | expression "To" expression | "Is" case_compare_op expression ;
case_compare_op    = "=" | "<>" | "<" | "<=" | ">" | ">=" ;

for_stmt           = "For" identifier "=" expression "To" expression ("Step" expression)?
                     logical_line* "Next" identifier? ;
for_each_stmt      = "For" "Each" identifier "In" expression logical_line* "Next" identifier? ;
do_loop_stmt       = "Do" loop_condition? logical_line* "Loop" loop_condition? ;
loop_condition     = ("While" | "Until") expression ;
while_wend_stmt    = "While" expression logical_line* "Wend" ;
with_stmt          = "With" expression logical_line* "End" "With" ;

on_error_stmt      = "On" "Error" ("GoTo" (LABEL | LINE_NUMBER | "0") | "Resume" "Next") ;
resume_stmt        = "Resume" ("Next" | LABEL | LINE_NUMBER)? ;
goto_stmt          = "GoTo" (LABEL | LINE_NUMBER) ;
gosub_stmt         = "GoSub" (LABEL | LINE_NUMBER) ;
return_stmt        = "Return" ;
exit_stmt          = "Exit" ("Sub" | "Function" | "Property" | "For" | "Do") ;
erase_stmt         = "Erase" lvalue ("," lvalue)* ;
redim_stmt         = "ReDim" "Preserve"? redim_item ("," redim_item)* ;
redim_item         = lvalue array_rank ;
raise_event_stmt   = "RaiseEvent" identifier argument_syntax? ;
local_decl_stmt    = dim_decl | static_decl ;
empty_stmt         = ;
```

## Expressions

Expression parsing is Pratt/precedence based. The grammar below names the layers for coverage and
fixtures; the parser does not need to implement it as recursive functions per layer.

```ebnf
expression         = comparison_expr ;
comparison_expr    = concat_expr (compare_op concat_expr)* ;
compare_op         = "=" | "<>" | "<" | "<=" | ">" | ">=" | "Is" | "Like" ;
concat_expr        = additive_expr ("&" additive_expr)* ;
additive_expr      = multiplicative_expr (("+" | "-") multiplicative_expr)* ;
multiplicative_expr = unary_expr (("*" | "/" | "\\") unary_expr)* ;
unary_expr         = ("+" | "-" | "Not") unary_expr | exponent_expr ;
exponent_expr      = postfix_expr ("^" unary_expr)? ;
postfix_expr       = primary_expr postfix_part* ;
postfix_part       = member_part | bang_part | index_or_call_part ;
member_part        = "." identifier ;
bang_part          = "!" identifier ;
index_or_call_part = "(" argument_list? ")" ;

primary_expr       = literal
                   | identifier
                   | "New" type_name
                   | "Nothing"
                   | parenthesized_expr
                   | type_of_expr ;
parenthesized_expr = "(" expression ")" ;
type_of_expr       = "TypeOf" expression "Is" type_name ;

argument_syntax    = argument_list | "(" argument_list? ")" ;
argument_list      = argument ("," argument)* ;
argument           = named_argument | expression? ;
named_argument     = identifier ":=" expression ;
callable_expr      = postfix_expr ;
lvalue             = postfix_expr ;
```

## Lexical Token Classes

```ebnf
identifier         = IDENTIFIER | BRACKETED_IDENTIFIER ;
type_name          = qualified_identifier | builtin_type ;
qualified_identifier = identifier ("." identifier)* ;
builtin_type       = "Boolean" | "Byte" | "Currency" | "Date" | "Decimal" | "Double"
                   | "Integer" | "Long" | "LongLong" | "LongPtr" | "Object" | "Single"
                   | "String" | "Variant" ;
literal            = STRING_LITERAL | DATE_LITERAL | NUMERIC_LITERAL | "True" | "False"
                   | "Empty" | "Null" ;
```

## Coverage Residuals

This scaffold intentionally leaves detailed rows for later grammar/coverage beads:

- preprocessor grammar (`#If`, `#ElseIf`, `#Else`, `#End If`, `#Const`);
- attributes and designer/module metadata details;
- full event/Implements/static semantics beyond syntactic recognition;
- full external reference grammar and host project storage shapes;
- userform/control declarations and runtime forms behavior;
- parser recovery nodes for incomplete IDE edit states.

## Checks

- Public MS-VBAL reference verified on Microsoft Learn on 2026-06-01.
- Existing repo language evidence and parser surface inspected.
- EBNF scaffold consistency check: all lower-snake-case RHS production references have a matching
  LHS definition.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.
