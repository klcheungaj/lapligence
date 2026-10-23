// Decision A10-D2: without the negative timing check option a negative
// $setuphold or $recrem limit counts as 0 (llg warns once per check) and the
// explicit delayed signals are copies of their signals.
//
// IEEE 1800-2009 31.9 (SystemVerilog-1800-2009.txt L52283-52284): "Both the
// $setuphold and $recrem timing checks can accept negative values when the
// negative timing check option is enabled."; 31.9.4 L52580-52584: "It is
// possible models written to accept negative timing check values with
// delayed reference and/or delayed data signals can be run without this
// invocation option enabled. In this circumstance, the delayed reference and
// data signals become copies of the original reference and data signals."
`timescale 1ns/1ns
module m(input r, input d); reg n; wire dr, dd;
  specify $setuphold(posedge r, d, -3, 8, n, , , dr, dd); endspecify
endmodule
module q(input r, input d); reg n; wire dr, dd;
  specify $recrem(posedge r, posedge d, 8, -3, n, , , dr, dd); endspecify
endmodule
module tb;
  reg r, d, rq, dq;
  m u(r, d);
  q v(rq, dq);
  initial $monitor("%0t r=%b dr=%b d=%b dd=%b n=%b | r=%b dr=%b d=%b dd=%b n=%b", $time,
                   r, u.dr, d, u.dd, u.n, rq, v.dr, dq, v.dd, v.n);
  initial begin
    r = 0; d = 0; rq = 0; dq = 0;
    #100 r = 1; rq = 1;  // 100
    #2 d = 1; dq = 1;    // 102: in the hold and recovery windows [100, 108)
    #3 d = 0;            // 105: in the hold window of u again
    #10 $finish;
  end
endmodule
