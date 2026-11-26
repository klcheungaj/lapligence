`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; real x; int q[$]; } r_t;
  logic clk = 0;
  task automatic show(ref r_t v);
    #1 $display("ref %s %0d %0d", v.s, v.n, v.q.size());
    v.s = {v.s, "!"};
  endtask
  initial begin
    // (A) a loop body record, written by the parent after the fork
    for (int i = 0; i < 3; i++) begin
      automatic r_t r = '{s: "a", n: i, x: 1.5, q: '{i}};
      fork
        begin
          #2 $display("A %s %0d %0.1f %p", r.s, r.n, r.x, r.q);
          r.n += 10;
          $display("A' %0d", r.n);
        end
      join_none
      r.s = {r.s, "late"};
      r.q.push_back(7);
    end
    #5;
    // (B) a ref actual, and a joined fork inside a detached one
    repeat (2) begin
      automatic r_t w;
      w.n = 0;
      fork
        show(w);
        fork
          #2 $display("B joined %s %0d", w.s, w.n);
        join
      join_none
      #1 w.s = "go";
      w.n = 5;
      #3;
      $display("B after %s", w.s);
    end
    // (C) a declaration inside a branch that runs again, with a nested fork
    for (int i = 0; i < 2; i++) fork
      begin
        automatic r_t c;
        c.n = i;
        c.s = "c";
        fork
          #1 $display("C %s%0d", c.s, c.n);
        join_none
        c.s = "changed";
      end
    join_none
    #3;
    // (D) whole-record copies
    for (int i = 0; i < 2; i++) begin
      automatic r_t d;
      r_t e;
      d.n = i; d.s = "d";
      fork
        begin
          #1 e = d;
          $display("D %s %0d", e.s, e.n);
          d = '{s: "z", n: 99, x: 0.0, q: '{}};
        end
      join_none
    end
    #3;
    clk = 1; #1 clk = 0; #1 clk = 1;
    #4 $finish;
  end
  // (E) an always block
  always @(posedge clk) begin
    automatic r_t a;
    a.n = $time;
    a.s = "e";
    fork
      #1 $display("E %s %0d", a.s, a.n);
    join_none
  end
endmodule
