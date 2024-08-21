


function Person ( ... ) local i = 0 local args = { ... } local function iota_impl ( ) i = i + 1 return args [ i ] end return { 
   name = iota_impl ( ) , 
   age = iota_impl ( ) , 
} end 

local p = Person ( "Ian" , 15 ) 
print ( p .name , p .age ) 
