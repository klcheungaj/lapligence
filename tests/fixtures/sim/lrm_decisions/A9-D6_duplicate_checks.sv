// Decision A9-D6: of two checks of one kind with the same events only the
// one with the larger limit is simulated (the first of equal limits); the
// other never reports or changes its notifier. llg warns when it generates
// the model.
//
// IEEE 1800-2009 31.3.1 (SystemVerilog-1800-2009.txt L50983-50985) defines
// each check on its own and does not address duplicates; keeping one is
// industry-standard behaviour.
`timescale 1ns/1ns
module du(input d, input c); reg n10, n5;
  specify $setup(d, posedge c, 10, n10); $setup(d, posedge c, 5, n5); endspecify
endmodule
module dr(input d, input c); reg n10, n5;
  specify $setup(d, posedge c, 5, n5); $setup(d, posedge c, 10, n10); endspecify
endmodule
module tb;
  reg c, d;
  du k(d, c); dr r(d, c);
  initial $monitor("%0t first=%b%b reordered=%b%b", $time, k.n10, k.n5, r.n10, r.n5);
  initial begin
    c = 0; d = 0;
    #97 d = 1;
    #3 c = 1;                    // 100: 3 before, within both limits
    #10 c = 0; d = 0;
    #3 c = 1;                    // 113: 3 before
    #10 $finish;
  end
endmodule
