// RTL-018 library `rtl` candidate for rtl018_pick.
module rtl018_pick(output [7:0] v);
  assign v = 8'd11;
  initial $display("pick %l");
endmodule
