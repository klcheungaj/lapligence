// SIM-032 A01: continuous assignments in a program are scheduled in the
// Reactive region (IEEE 1800-2009 24.3, 24.3.1, 24.3.2).
module tb;
  logic r;
  wire dw1, dw2, y, z;
  initial begin
    r = 0;
    #3 $display("t=%0d y=%b z=%b", $time, y, z);
    #7 r = 1;
    #1 $display("t=%0d y=%b z=%b", $time, y, z);
    #2 $display("t=%0d y=%b z=%b", $time, y, z);
    #7 $finish;
  end
  assign dw1 = r;
  p p_i(dw2, dw1, y, z);
  always @(dw2) $display("dw2 is %b t=%0d", dw2, $time);
endmodule

program p(output pw2, input pw1, output wire py, output wire pz);
  assign pw2 = pw1;
  assign #2 py = pw1;
  wire pn = ~pw1;
  assign pz = pn;
endprogram
