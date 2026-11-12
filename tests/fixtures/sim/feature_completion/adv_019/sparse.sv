// ADV-019 catalog gaps, sparse fixed-size arrays: the VPI application
// sparse.c reads, finds, watches and writes the elements of unpacked arrays
// that llg keeps in a keyed container (more than 4096 cells, or more bits
// than one packed value holds) as it does for dense arrays (IEEE 1800-2009
// 37.17, 37.20, 38.15, 38.16, 38.34, 38.35, 38.36.1). See readme.md for the
// expected output.
`timescale 1ns/1ns
module tb;
  reg [7:0] mem [0:8191];
  int big [4999:0];
  logic [3:0] two [0:99][0:99];
  logic [255:0] wide [0:4095];
  initial begin
    mem[5] = 8'ha5;
    big[4000] = 77;
    two[3][4] = 4'h9;
    wide[7] = 256'h1;
    $sparse(1);
    $display("mem[6]=%h big[10]=%0d two[1][2]=%h wide[9]=%h mem[0]=%h", mem[6], big[10],
             two[1][2], wide[9][7:0], mem[0]);
    $display("mem[100]=%h mem[101]=%h mem[102]=%h", mem[100], mem[101], mem[102]);
    #1 mem[7] = 8'h77;
    #1 big[3] = 3;
    #1 $sparse(2);
    $finish;
  end
  always @(mem[6]) $display("%0t: hdl saw mem[6]=%h", $time, mem[6]);
endmodule
