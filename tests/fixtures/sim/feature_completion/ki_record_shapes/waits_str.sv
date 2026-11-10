// Waits on members of a record with a string member and a member array of N
// cells, held in a static subroutine variable and in a class property: one
// waiter per row, each resumed by one store at its own time. Hand-written
// (not part of the generated grid); see readme.md.
`ifndef N
`define N 16
`endif
typedef struct { int a; string s; int arr[`N]; int b; } rec_t;
class C; rec_t r; endclass
module tb;
  timeunit 1ns; timeprecision 1ns;
  C h = new;
  task run();
    static rec_t r;
    static int k = 1;
    fork
      begin @(r.s); $display("row 1 %0t %s", $time, r.s); end
      begin @(r.b); $display("row 2 %0t %0d", $time, r.b); end
      begin wait (r.arr[1] == 5); $display("row 3 %0t %0d", $time, r.arr[1]); end
      begin @(r.arr[k + 1]); $display("row 4 %0t %0d", $time, r.arr[2]); end
      begin wait (r.a == 2 && r.s == "t"); $display("row 5 %0t", $time); end
      begin @(posedge r.a); $display("row 10 %0t %0d", $time, r.a); end
      begin
        #1 r.s = "s";
        #1 r.b = 3;
        #1 r.arr[k] = 5;
        #1 r.arr[2] = 6;
        #1 r.a = 2;
        #1 r.s = "t";
        #5 r.a = 3;
      end
    join
  endtask
  initial begin wait (h.r.a == 1); $display("row 6 %0t", $time); end
  initial begin @(h.r.s); $display("row 7 %0t %s", $time, h.r.s); end
  initial begin @(h.r.arr[1]); $display("row 8 %0t %0d", $time, h.r.arr[1]); end
  initial begin @(h.r.b); $display("row 9 %0t %0d", $time, h.r.b); end
  initial begin
    #7 h.r.a = 1;
    #1 h.r.s = "u";
    #1 h.r.arr[1] = 9;
    #1 h.r.b = 4;
  end
  initial begin
    run();
    #1 $display("row 11 %0t end", $time);
    $finish(0);
  end
endmodule
