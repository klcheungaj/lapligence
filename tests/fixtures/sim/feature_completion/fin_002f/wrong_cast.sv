// $cast with values, objects and types that do not fit (6.24.2).
`timescale 1ns/1ns
module tb;
  typedef enum logic [2:0] {A = 1, B = 3, C = 5} e_t;
  typedef enum bit [1:0] {X0, X1, X2} f_t;
  typedef struct packed { logic [1:0] h; e_t e; } ps_t;
  typedef class d1;
  class base;
    int v = 1;
    function int down();
      d1 d;
      return $cast(d, this);
    endfunction
  endclass
  class d1 extends base; int w = 2; endclass
  class d2 extends base; int z = 3; endclass
  class gen #(type T = int); T t; endclass
  class g2 extends gen #(real); endclass
  e_t e; f_t f; ps_t ps; base b, b0; d1 x1; d2 x2;
  int r, ctx; real re; string s; logic [7:0] l8; longint li;
  semaphore sem = new(1);
  mailbox mb = new;
  gen #(int) gi; gen #(real) gr; g2 gg;
  task run(int c);
    case (c)
      0: begin r = $cast(e, 3); $display("%0d %s", r, e.name()); end
      1: begin e = A; r = $cast(e, 4); $display("%0d %s", r, e.name()); end
      2: begin e = A; $cast(e, 4); $display("after task %s", e.name()); end
      3: begin e = A; $cast(e, 5); $display("after task %s", e.name()); end
      4: begin e = A; r = $cast(e, 'x); $display("%0d %s", r, e.name()); end
      5: begin e = A; r = $cast(e, 11); $display("%0d %s", r, e.name()); end
      6: begin e = A; r = $cast(e, -5); $display("%0d %s", r, e.name()); end
      7: begin f = X1; r = $cast(f, 3); $display("%0d %s", r, f.name()); end
      8: begin f = X1; r = $cast(f, 6); $display("%0d %s", r, f.name()); end
      9: begin re = 3.4; r = $cast(e, re); $display("%0d %s", r, e.name()); end
      10: begin re = 4.4; e = A; r = $cast(e, re); $display("%0d %s", r, e.name()); end
      11: begin b = new; r = $cast(x1, b); $display("%0d %0d", r, x1 == null); end
      12: begin x1 = new; b = x1; r = $cast(x2, b); $display("%0d %0d", r, x2 == null); end
      13: begin x1 = new; b = x1; x2 = new; x2.z = 9; r = $cast(x2, b); $display("%0d %0d", r, x2.z); end
      14: begin x1 = new; b = x1; $cast(x2, b); $display("after %0d", x2 == null); end
      15: begin x1 = new; b = x1; x1 = null; $cast(x1, b); $display("after %0d", x1.w); end
      16: begin b = null; x1 = new; r = $cast(x1, b); $display("%0d %0d", r, x1 == null); end
      17: begin x1 = new; r = $cast(b, x1); $display("%0d %0d", r, b.v); end
      18: begin gg = new; gr = gg; r = $cast(gg, gr); $display("%0d", r); end
      19: begin gr = new; r = $cast(gg, gr); $display("%0d", r); end
      20: begin l8 = 8'b1010_1011; r = $cast(ps, l8); $display("%0d %b", r, ps); end
      21: begin l8 = 8'hff; r = $cast(li, l8); $display("%0d %0d", r, li); end
      22: begin e = A; r = $cast(e, 64'h1_0000_0003); $display("%0d %s", r, e.name()); end
      23: begin e = A; if (!$cast(e, 7)) $display("failed %s", e.name()); else $display("ok"); end
      24: begin e_t q[$]; e_t t; q = '{A, B}; r = $cast(t, q[1]); $display("%0d %s", r, t.name()); end
      25: begin re = 1.0; r = $cast(re, 7); $display("%0d %f", r, re); end
      26: begin r = $cast(r, 2.6); $display("%0d", r); end
      27: begin e = A; r = $cast(e, $urandom_range(3, 3)); $display("%0d %s", r, e.name()); end
      28: begin void'($cast(e, 2)); $display("void %s", e.name()); end
      29: begin e = B; r = $cast(e, 3'b0z1); $display("%0d %s", r, e.name()); end
      30: begin f = X2; r = $cast(f, 2'bx1); $display("%0d %0d", r, f); end
      31: begin x1 = new; b = x1; b0 = new; $display("%0d %0d", b.down(), b0.down()); end
      32: begin b = new; r = $cast(r, b); $display("%0d", r); end
      33: begin b = new; re = 2.5; r = $cast(re, b); $display("%0d %0.1f", r, re); end
      34: begin b = new; s = "kept"; r = $cast(s, b); $display("%0d %s", r, s); end
      35: begin b = new; e = B; $cast(e, b); $display("after %s", e.name()); end
      36: begin b = new; b0 = b; r = $cast(b, 5); $display("%0d %0d", r, b == b0); end
      37: begin b = new; b0 = b; r = $cast(b, s); $display("%0d %0d", r, b == b0); end
      38: begin r = $cast(b, sem); $display("%0d %0d", r, b == null); end
      39: begin r = $cast(b, mb); $display("%0d %0d", r, b == null); end
      40: begin process p; p = process::self(); r = $cast(b, p); $display("%0d %0d", r, b == null); end
      default: ;
    endcase
  endtask
  initial begin
    void'($value$plusargs("ctx=%d", ctx));
    fork begin run(ctx); $display("went on"); end join_none
    #5 $display("main ends");
    $finish;
  end
endmodule
