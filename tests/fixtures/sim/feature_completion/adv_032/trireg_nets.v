module tri_cell (output trireg y, input a, input en);
  assign y = en ? a : 1'bz;
endmodule

module tb;
  trireg t1;
  trireg (small) t2;
  trireg (large) #(1, 2, 30) t3;
  trireg (medium) [3:0] t4;
  trireg t5 [0:1];
  wire w;
  reg a, en;
  tri_cell u (.y(w), .a(a), .en(en));
  assign t1 = en ? a : 1'bz;
  initial begin
    a = 1'b1;
    en = 1'b1;
    #1 en = 1'b0;
    #1 $display("%b %b %b %b %b", t1, t2, t3, t4, w);
    $finish;
  end
endmodule
