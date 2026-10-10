// SIM-036 A01: deferred assertions in static and automatic tasks, in a class
// method and in a function called from a clocked process and from forked
// processes (IEEE 1800-2009 16.4, 16.4.1, 16.4.5). Values passed by value
// are the values when the assertion executed, even after the subroutine
// changed or released them.
module tb;
  int v;
  logic clk = 1'b0;
  int count = 0;

  task report(input int a);
    $display("%0d static task a=%0d v=%0d", $time, a, v);
  endtask

  task check_static(input int q);
    assert #0 (q > 10) else report(q);
  endtask

  task automatic auto_report(input int a, input int b);
    $display("%0d automatic task a=%0d b=%0d", $time, a, b);
  endtask

  task automatic check_auto(input int q);
    int doubled;
    doubled = q * 2;
    assert #0 (q > 10) else auto_report(q, doubled);
    doubled = 99;
    q = 77;
  endtask

  class Limit;
    int lim;
    function new(int l);
      lim = l;
    endfunction
    function void check(int q);
      assert #0 (q < lim) else $display("%0d method q=%0d lim=%0d", $time, q, lim);
    endfunction
  endclass

  function automatic void odd(input int c);
    assert #0 (c % 2 == 0) else $display("%0d odd c=%0d", $time, c);
  endfunction

  always @(posedge clk) begin
    count <= count + 1;
    odd(count);
  end

  Limit l;
  int g;

  initial begin
    v = 1;
    check_static(3);
    v = 2;
    check_auto(4);
    l = new(5);
    l.check(3);
    l.check(9);
    l.lim = 100;
    #1;
    // Forked processes call one function; the second process re-checks
    // after an event control, which flushes its own earlier failure only.
    g = 1;
    fork
      odd(g);
      begin
        odd(g + 2);
        @(g);
        odd(g);
      end
    join_none
    #0 g = 4;
    #1;
    clk = 1'b1;
    #1 clk = 1'b0;
    #1 clk = 1'b1;
    #1 $finish(0);
  end
endmodule
