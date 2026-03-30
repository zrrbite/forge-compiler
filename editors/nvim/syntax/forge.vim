" Vim syntax file
" Language: Forge
" Maintainer: zrrbite
" Latest Revision: 2026-03-30

if exists("b:current_syntax")
  finish
endif

" Keywords
syn keyword forgeKeyword fn let mut struct impl trait enum use pub mod
syn keyword forgeKeyword comptime spawn where
syn keyword forgeControl if else while for in match return break continue
syn keyword forgeSelf self Self

" Booleans
syn keyword forgeBool true false

" Constants
syn keyword forgeConstant None PI E

" Constructors / variants
syn keyword forgeConstructor Ok Err Some HashMap

" Primitive types
syn keyword forgeType i8 i16 i32 i64 i128 u8 u16 u32 u64 u128
syn keyword forgeType f32 f64 bool str usize isize

" User-defined types (PascalCase)
syn match forgeType "\v<[A-Z][A-Za-z0-9_]*>"

" Function definitions
syn match forgeFunction "\v<fn\s+\zs[a-z_][a-zA-Z0-9_]*\ze\s*[(<]"

" Function calls
syn match forgeFuncCall "\v<[a-z_][a-zA-Z0-9_]*\ze\s*\("

" Numbers
syn match forgeNumber "\v<\d+\.\d+>"
syn match forgeNumber "\v<\d+>"

" Operators
syn match forgeOperator "\V->"
syn match forgeOperator "\V=>"
syn match forgeOperator "\V.."
syn match forgeOperator "\V..="
syn match forgeOperator "\V?"
syn match forgeOperator "\V@"
syn match forgeOperator "\V::"

" Comments
syn match forgeComment "\V//\.\*$"

" Strings with interpolation
syn region forgeString start='"' end='"' contains=forgeEscape,forgeInterp
syn match forgeEscape contained "\v\\[nrt\\\"{}0]"
syn region forgeInterp contained matchgroup=forgeInterpDelim start="{" end="}" contains=TOP

" Highlighting links
hi def link forgeKeyword Keyword
hi def link forgeControl Conditional
hi def link forgeSelf Identifier
hi def link forgeBool Boolean
hi def link forgeConstant Constant
hi def link forgeConstructor Type
hi def link forgeType Type
hi def link forgeFunction Function
hi def link forgeFuncCall Function
hi def link forgeNumber Number
hi def link forgeOperator Operator
hi def link forgeComment Comment
hi def link forgeString String
hi def link forgeEscape SpecialChar
hi def link forgeInterpDelim Special

let b:current_syntax = "forge"
