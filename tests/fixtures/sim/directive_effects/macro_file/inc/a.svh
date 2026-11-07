`define WHERE_A `__FILE__
`include "b.svh"
function void show_a(); $display("a %s", `__FILE__); endfunction
