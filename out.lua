
 local ffi = require("ffi") 

 ffi .cdef[[
     typedef struct{
         int x; 
         int y; 
    } Point; 
]] 

 

 

 

 local arr = ffi .new("int" .. "[" .. 10000 .. "]") 
 for i = 0, 10000 do 
     arr[i] = math .random(1, 1000) 
 end 
 local sum = 0 
 for i = 0, 10000 do 
     var = sum + arr[i] 
 end 

 print(sum)