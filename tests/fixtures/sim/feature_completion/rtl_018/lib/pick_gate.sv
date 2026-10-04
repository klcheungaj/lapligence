// RTL-018 library `gate` candidate for rtl018_pick.
module rtl018_pick(output [7:0] v);
  assign v = 8'd22;
  initial $display("pick %l");
endmodule
