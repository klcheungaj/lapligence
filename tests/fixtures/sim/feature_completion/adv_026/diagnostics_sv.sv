// ADV-026 A02 with SystemVerilog storage: an enumerated memory, selected
// slices (guard cells on both sides must keep their value), dynamic arrays,
// queues, associative arrays and two-state words. The categories follow IEEE
// 1800-2009 21.4, 21.4.1 and 21.4.2; the strings take the format of memory
// files (D.14). The tag at the end of each call line lets the test locate it.
module tb;
  typedef enum logic [1:0] { RED = 0, GREEN = 1, BLUE = 2 } color_t;
  color_t ce [0:3];
  logic [7:0] g [0:11];
  logic [7:0] m8 [0:0];
  bit [7:0] b2 [0:3];
  int dyn [];
  int empty [];
  int fifo [$];
  int aa [int];
  integer i;

  task fresh;
    for (i = 0; i < 12; i = i + 1) g[i] = 8'h5a;
  endtask

  task show;
    input string tag;
    $display("%s %h %h %h %h %h %h %h %h %h %h %h %h", tag, g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7], g[8], g[9], g[10], g[11]);
  endtask

  initial begin
    $sreadmemh(ce, 0, 3, "1 2 3 10"); // E1
    $display("E1 %b %b %b %b", ce[0], ce[1], ce[2], ce[3]);

    fresh; $sreadmemh(g[4:7], 4, 7, "11 22 33 44 55 66"); show("S1"); // S1
    fresh; $sreadmemh(g[4:7], 4, 9, "11"); show("S2"); // S2
    fresh; $sreadmemh(g[4:7], 4, 7, "11 @3 22"); show("S3"); // S3
    fresh; $sreadmemh(g[4:7], 7, 4, "11 22 33 44"); show("S4"); // S4

    empty = new[0];
    $sreadmemh(empty, 0, 0, "1"); // R1
    $display("R1 size=%0d", empty.size());
    dyn = new[3];
    dyn[0] = 7; dyn[1] = 8; dyn[2] = 9;
    $sreadmemh(dyn, 0, 5, "1 2 3 4 5 6"); // R2
    $display("R2 %0d %0d %0d size=%0d", dyn[0], dyn[1], dyn[2], dyn.size());
    $sreadmemh(dyn, 0, 2, "1 2 3 4 5 6"); // R3
    $display("R3 %0d %0d %0d size=%0d", dyn[0], dyn[1], dyn[2], dyn.size());
    fifo = '{7, 8, 9};
    $sreadmemh(fifo, 2, 1, "a b c"); // R4
    $display("R4 %0d %0d %0d size=%0d", fifo[0], fifo[1], fifo[2], fifo.size());

    $sreadmemh(aa, 0, 2, "1 2 3 4"); // A1
    $display("A1 size=%0d %0d %0d %0d", aa.size(), aa[0], aa[1], aa[2]);

    b2 = '{default: 8'h77};
    $sreadmemh(b2, 0, 3, "1x 2z 3"); // W1
    $display("W1 %h %h %h %h", b2[0], b2[1], b2[2], b2[3]);
    $sreadmemh(m8, 0, 0, "123"); // W2
    $display("W2 %h", m8[0]);
    $finish;
  end
endmodule
