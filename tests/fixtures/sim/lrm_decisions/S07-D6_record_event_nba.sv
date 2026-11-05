// S07-D6: an event-controlled nonblocking write to a member of a record element
// keeps the element position and the value of the issue, and reaches the
// element that holds that position when the control fires.
// 10.4.2: "If variable_lvalue requires an evaluation, it shall be evaluated at
// the same time as the expression on the right-hand side."
// 9.4.5: "the right-hand expression shall be evaluated before the delay".
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } rec_t;
  rec_t r[3];
  rec_t empty;
  string x = "first";
  int k = 1;
  event e;
  initial begin
    r[k].s <= @(e) x;
    r[k].n <= @(e) 5;
    k = 2;
    x = "second";
    r[1] = empty;
    #1 -> e;
    #1 $display("[%s] %0d [%s] %0d", r[1].s, r[1].n, r[2].s, r[2].n);
    $finish(0);
  end
endmodule
