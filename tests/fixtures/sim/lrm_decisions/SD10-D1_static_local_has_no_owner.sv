// Decision SD10-D1: the storage declared in a task or function (formals,
// result and locals, also of static lifetime) is no variable that an
// always_comb, always_latch or always_ff procedure calling it owns, so any
// number of procedures may call the subroutine.
//
// IEEE 1800-2009 9.2.2.2.2 (SystemVerilog-1800-2009.txt L11213-11216):
//   "Variables on the left-hand side of assignments within an always_comb
//   procedure, including variables from the contents of a called function,
//   shall not be written to by any other processes". Read literally the
//   sentence would allow one always_comb per static function; a variable of
//   the design that two of them write through a function stays an error.
module tb;
  function [7:0] reversed(input [7:0] v);
    integer i;
    for (i = 0; i < 8; i = i + 1) reversed[i] = v[7 - i];
  endfunction
  function int queued(int v);
    int q[$];
    q = {};
    q.push_back(v);
    return q.sum();
  endfunction
  function int text(int v);
    string s;
    s = $sformatf("%0d", v);
    return s.len();
  endfunction
  function int cells(int v);
    int f[4];
    f[v & 3] = v;
    return f[v & 3];
  endfunction
  task plus(input int v, output int o);
    int k;
    k = v;
    o = k + 1;
  endtask

  int x;
  bit clk;
  logic [7:0] r1, r2, ff1, ff2;
  int q1, q2, t1, t2, c1, c2, p1, p2;
  wire [7:0] w;

  always_comb r1 = reversed(x[7:0]);
  always_comb r2 = reversed(x[7:0] + 8'd1);
  assign w = reversed(x[7:0] + 8'd2);
  always_comb q1 = queued(x);
  always_comb q2 = queued(x + 1);
  always_comb t1 = text(x);
  always_comb t2 = text(x + 100);
  always_comb c1 = cells(x);
  always_comb c2 = cells(x + 1);
  always_comb plus(x, p1);
  always_comb plus(x + 1, p2);
  always_ff @(posedge clk) ff1 <= reversed(8'd1);
  always_ff @(posedge clk) ff2 <= reversed(8'd2);

  initial begin
    for (int n = 3; n < 6; n += 2) begin
      x = n;
      #1 clk = 1;
      #1 clk = 0;
      $display("%b %b %b %b %b", r1, r2, w, ff1, ff2);
      $display("%0d %0d %0d %0d %0d %0d %0d %0d", q1, q2, t1, t2, c1, c2, p1, p2);
    end
    $finish;
  end
endmodule
