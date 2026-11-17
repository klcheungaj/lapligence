// ADV-019 callback kinds (FIN-001 V3 family (e)): callback_kinds.c places
// cbValueChange records on the chandle variables h and h2 and prints each
// call, and tries the callbacks llg refuses; see readme.md.
`timescale 1ns / 1ns
module tb;
  import "DPI-C" function chandle cbk_make(input int n);
  chandle h, h2;
  int da[];
  int q[$];
  int aa[int];
  string s;
  logic [3:0] a, b;
  wire [3:0] w;
  wire y;
  assign w = a & b;
  and g1(y, a[0], b[0]);
  initial begin
    a = 4'h0;
    b = 4'h0;
    $cbk(0);
    #1 h = cbk_make(5);
    da = new[2];
    q.push_back(1);
    aa[3] = 1;
    s = "ab";
    a = 4'h3;
    b = 4'h1;
    #1 h = cbk_make(5);
    #1 h = null;
    #1 begin
      h = cbk_make(7);
      h = null;
    end
    #1 begin
      h = cbk_make(8);
      h = cbk_make(9);
    end
    #1 h2 = h;
    #1 $cbk(1);
    #1 h = cbk_make(10);
    #1 $cbk(2);
    $display("done %0d %0d %0d %0d %0d %h %b", h == h2, da.size(), q.size(), aa.num(), s.len(), w, y);
    $finish;
  end
endmodule
