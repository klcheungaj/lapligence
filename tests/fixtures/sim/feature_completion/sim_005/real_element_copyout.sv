// SIM-005: output and inout actuals that select a real or shortreal array
// element copy back into that numeric cell (IEEE 1800-2009 13.3, 13.5.1).
module tb;
  real a[2][4];
  shortreal s[3];
  int i;
  int wakes;

  task automatic bump(inout real x, input real d);
    x = x + d;
  endtask

  task automatic put(output shortreal y, input real d);
    y = shortreal'(d);
  endtask

  task automatic move(inout real x);
    i = 0;
    x = x + 10.0;
  endtask

  function automatic real twice(inout real z);
    z = z * 2.0;
    return z;
  endfunction

  initial begin
    i = 1;
    for (int k = 0; k < 5; k++) bump(a[i][k % 4], 0.25);
    put(s[i + 1], 1.0 / 3.0);
    $display("bump %.2f %.2f", a[1][0], a[1][1]);
    $display("put %h", $shortrealtobits(s[2]));
    $display("twice %.2f", twice(a[1][1]));
    $display("after %.2f", a[1][1]);
    move(a[i][2]);
    $display("move %.2f %.2f i %0d", a[1][2], a[0][2], i);
    i = 5;
    bump(a[1][i], 1.0);
    put(s[i], 2.0);
    $display("invalid %.2f %.2f %.2f", a[1][0], a[1][1], a[1][3]);
    #2;
    $display("timed %.2f wakes %0d", a[1][3], wakes);
    $finish(0);
  end

  initial forever @(a[1][3]) wakes++;
  initial #1 bump(a[1][3], 1.5);
endmodule
