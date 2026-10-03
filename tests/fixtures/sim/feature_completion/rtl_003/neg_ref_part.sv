// SV2009 13.5.2: a packed part-select is not a subroutine ref actual.
module tb;
 logic [7:0] value;
 task automatic change(ref logic [3:0] x); x = 0; endtask
 initial begin change(value[5:2]); $finish(0); end
endmodule
