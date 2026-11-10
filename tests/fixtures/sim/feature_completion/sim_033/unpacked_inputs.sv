`timescale 1ns/1ns
// Unpacked fixed-size clockvars sample their whole value (SV 14.3, 14.13):
// fixed arrays, a multidimensional array, a mixed-state unpacked structure,
// an element of a structure array bound by expression,
// every skew mode, and whole-value, element and member reads.
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  typedef struct { int a; logic [3:0] b; } st_t;
  int arr [2] = '{1, 2};
  logic [3:0] grid [0:1][0:2];
  st_t st = '{3, 4'h5};
  st_t recs [2];
  int t [2];
  st_t ts;
  int i = 1;
  clocking cb @(posedge clk);
    input arr, grid, st;
    input ren = arr;
    input el = recs[1];
    input #2 sk = arr;
    input #0 a0 = arr;
  endclocking
  initial begin
    foreach (grid[j, k]) grid[j][k] = 4'(unsigned'(j * 3 + k));
    recs[1] = '{6, 4'h7};
    #7 arr[1] = 20; st.a = 30; grid[1][2] = 4'hf; recs[1].a = 66;
    #5 arr[0] = 11;
    #8 arr = '{5, 6}; st.b = 4'h9;
    #4 arr[0] = 44;
    #2 i = 0;
  end
  always @(cb) begin
    t = cb.arr;
    ts = cb.st;
    $display("%0t arr %0d %0d grid %h %h st %0d %h ren %0d el %0d %h", $time,
             cb.arr[0], cb.arr[1], cb.grid[0][1], cb.grid[1][2], cb.st.a, cb.st.b,
             cb.ren[1], cb.el.a, cb.el.b);
    $display("%0t sk %0d %0d a0 %0d %0d dyn %0d copy %0d %0d %0d %h eq %0d size %0d", $time,
             cb.sk[0], cb.sk[1], cb.a0[0], cb.a0[1], cb.arr[i], t[0], t[1], ts.a, ts.b,
             cb.st == st, $size(cb.grid, 2));
    foreach (cb.arr[k]) $display("%0t foreach %0d %0d", $time, k, cb.arr[k]);
  end
  initial #36 $finish;
endmodule
