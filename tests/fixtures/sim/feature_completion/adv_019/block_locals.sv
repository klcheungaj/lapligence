// ADV-019 catalog gaps, stored automatics: the VPI application
// block_locals.c reads, finds by name and writes automatic arrays llg keeps
// in a keyed container, unpacked structures kept as record values or packed
// images, and the locals of a task's blocks while the task runs (IEEE
// 1800-2009 37.12, 37.17, 37.24, 37.39). See readme.md for the expected
// output.
`timescale 1ns/1ns
module tb;
  typedef struct { int a; string s; } rs_t;
  typedef struct { int a; int arr[3]; } ra_t;
  task automatic t(input int k);
    logic [127:0] wide [0:9000];
    int big [0:5000];
    rs_t one;
    ra_t ua;
    wide[1] = 128'h1234;
    big[4000] = 9;
    one.a = 1;
    one.s = "hi";
    ua.a = 3;
    ua.arr[1] = 7;
    begin : blk
      int bx;
      string bs;
      bx = 11;
      bs = "bb";
      $locals(1);
      $display("after: bx=%0d one.a=%0d one.s=%s ua.arr[1]=%0d big[3]=%0d wide[2]=%0h", bx,
               one.a, one.s, ua.arr[1], big[3], wide[2]);
    end
    for (int i = 0; i < 1; i++) begin
      int li;
      li = 5;
      $locals(2);
    end
    $locals(3);
  endtask
  initial begin
    t(1);
    $finish;
  end
endmodule
