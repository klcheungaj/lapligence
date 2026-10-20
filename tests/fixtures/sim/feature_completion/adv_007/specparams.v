`timescale 1ns/100ps
// ADV-007 A01/A02: specparams declared in the module body and inside the
// specify block give path delays; a later specparam may use an earlier one;
// min:typ:max takes the typical value; a negative delay is zero; real values
// scale to the precision. A module item may read a specify-block specparam.
module body_sp(input a, output y);
  specparam tpd = 3;
  specparam tpd2 = tpd + 1;
  assign y = a;
  specify
    (a => y) = (tpd, tpd2);
  endspecify
endmodule

module block_sp(input a, output y);
  assign y = a;
  specify
    specparam tr = 1.5, tf = 2:5:9;
    (a => y) = (tr, tf);
  endspecify
  initial $display("tr=%0.1f tf=%0d", tr, tf);
endmodule

module neg_sp(input a, output y);
  assign y = a;
  specify
    (a => y) = (-2, 3);
  endspecify
endmodule

module mtm(input a, output y);
  assign y = a;
  specify
    (a => y) = (1:2:3, 4:6:8);
  endspecify
endmodule

module tb;
  reg a;
  wire yb, yk, yn, ym;
  body_sp ub(.a(a), .y(yb));
  block_sp uk(.a(a), .y(yk));
  neg_sp un(.a(a), .y(yn));
  mtm um(.a(a), .y(ym));
  initial $monitor("%0.1f a=%b yb=%b yk=%b yn=%b ym=%b", $realtime, a, yb, yk, yn, ym);
  initial begin
    a = 0;
    #10 a = 1;
    #10 a = 0;
    #10 $finish;
  end
endmodule
