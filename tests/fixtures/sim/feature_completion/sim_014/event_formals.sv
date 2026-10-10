// SIM-014: mixed event lists (SV 9.4.2.1) and event formals of tasks,
// functions and class methods (SV 13.5, 15.5.5). An event formal shares the
// actual's synchronization object, so triggers and waits through the formal
// act on the caller's event; a function may compare event formals in an
// expression (SV 15.5.5.3).
`timescale 1ns / 1ns
module tb;
  logic clk = 0, clk2 = 0, sig = 0;
  event ev, ev2, ev3, a, b, c, got;
  int hits;

  class Base;
    int seen;
    virtual task wait_on(event e1);
      @e1 seen += 1;
    endtask
  endclass

  class Derived extends Base;
    virtual task wait_on(event e1);
      @e1 seen += 10;
    endtask
    task automatic chain(event e1, int d);
      if (d > 0) begin
        @(e1 or posedge clk2) seen += 100;
        chain(e1, d - 1);
      end
    endtask
    task later(event e1);
      ->> #1 e1;
    endtask
    task copy(output event o, input event i);
      o = i;
    endtask
    function bit same(event p, event q);
      return p == q;
    endfunction
  endclass

  Base bh;
  Derived d;

  task automatic waiter(event e1, int id);
    @e1 $display("%0t waiter %0d", $time, id);
  endtask

  task automatic rec(event e1, int depth);
    if (depth > 0) begin
      @e1 $display("%0t rec %0d", $time, depth);
      rec(e1, depth - 1);
    end
  endtask

  task automatic fire_later(event e1, int dly);
    #dly ->e1;
  endtask

  function automatic bit same(event p, event q);
    return p == q;
  endfunction

  function automatic bit live(event p);
    return p != null && p.triggered;
  endfunction

  initial forever begin
    @(posedge clk or ev or sig) begin
      hits++;
      $display("%0t mixed hit", $time);
    end
  end

  initial begin
    #1 clk = 1;
    #1 clk = 0;
    #1 ->ev;
    #1 sig = 1;
    #1 $display("%0t hits=%0d", $time, hits);
    fork
      waiter(ev3, 1);
      #1 rec(ev2, 3);
      begin
        #2 ->ev3;
        #1 ->ev2;
        #1 ->ev2;
        #1 fire_later(ev2, 1);
      end
    join
    $display("%0t same=%0d %0d live=%0d", $time, same(ev, ev), same(ev, ev2), live(ev2));
    d = new;
    bh = d;
    fork
      bh.wait_on(a);
      #1 ->a;
    join
    $display("%0t seen=%0d", $time, d.seen);
    fork
      d.chain(b, 2);
      begin
        #1 ->b;
        #1 clk2 = 1;
      end
    join
    $display("%0t seen=%0d", $time, d.seen);
    fork
      @c $display("%0t c", $time);
      d.later(c);
    join
    d.copy(got, a);
    $display("method same=%0d %0d %0d", d.same(got, a), d.same(a, b), d.same(got, null));
    $finish;
  end
endmodule
