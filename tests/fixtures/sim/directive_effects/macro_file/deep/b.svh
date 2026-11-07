`define WHERE_B `__FILE__
function void show_b(); $display("b %s macro a in b %s", `__FILE__, `WHERE_A); endfunction
`include "c.svh"
