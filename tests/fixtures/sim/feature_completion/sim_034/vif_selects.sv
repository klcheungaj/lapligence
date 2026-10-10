// SIM-034: selected destinations of synchronous drives through virtual
// interfaces and a modport view (IEEE 1800-2009 14.16 `clockvar select`,
// 25.5, 25.9). Element selects of a multidimensional packed clockvar, an
// element select followed by a part-select, ascending ranges and an indexed
// part-select are applied to the instance the handle names when each drive
// executes. The oracle is in readme.md.
interface sel_bus (input bit clk);
  logic [1:0][3:0] m = '0;
  logic [0:7] asc = '0;
  logic [7:0] v8 = '0;
  clocking sb @(posedge clk);
    output m, asc, v8;
  endclocking
  modport tb_mp (clocking sb);
endinterface

module tb;
  bit clk = 0;
  sel_bus bi (clk);
  sel_bus bj (clk);
  virtual sel_bus v;
  virtual sel_bus.tb_mp w;
  int k = 4;
  initial begin
    v = bj;
    w = bi;
    v.sb.m[1][2] <= 1'b1;
    v.sb.m[0] <= 4'h5;
    v.sb.m[1][1:0] <= 2'b11;
    v.sb.asc[0:3] <= 4'hA;
    v.sb.asc[6] <= 1'b1;
    v.sb.v8[k+:4] <= 4'hC;
    w.sb.v8 <= ##1 8'h3C;
    w.sb.asc[7] <= 1'b1;
    v = bi;
    k = 0;
    #1 clk = 1;
    #1 $display("bj m=%b asc=%b v8=%h", bj.m, bj.asc, bj.v8);
    $display("bi m=%b asc=%b v8=%h", bi.m, bi.asc, bi.v8);
    $finish;
  end
endmodule
