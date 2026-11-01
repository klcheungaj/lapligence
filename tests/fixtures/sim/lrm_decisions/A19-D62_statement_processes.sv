// Decision A19-D62: the process of a statement and the statement of
// `wait fork`.
// IEEE 1800-2009 37.56 (L60992) draws vpiProcess from every atomic
// statement and 37.58 (L61076) lists "process: initial, final, always"; the
// text gives no detail for a statement outside a process. 37.62 (L61177)
// draws vpiStmt from the waits class, which includes "wait fork", while
// Syntax 9-7 (L12323-12324) reads "wait ( expression ) statement_or_null |
// wait fork ;": `wait fork` has no statement.
// Decision: vpi_handle(vpiProcess, stmt) returns the initial, always or
// final construct whose body contains the statement at any depth, and NULL
// without an error for a statement of a task or function; a function call,
// an expression of the tf call class, refuses the relation with an error.
// vpi_handle(vpiStmt)
// of a `wait fork` returns NULL without an error, as a defined relation
// with no object does (decision A19-D26).
`timescale 1ns/1ns
module tb;
  int r, r2;
  logic clk = 0;
  task automatic bump();
    r = r + 1;
  endtask
  function int twice(input int v);
    return 2 * v;
  endfunction
  always @(posedge clk) r2 <= twice(r2);
  initial begin
    r = 0;
    if (r == 0) begin
      r = 1;
    end
    fork
      #1 r = 3;
    join_none
    wait fork;
    bump();
    r = twice(r);
    #1 $d62;
    $finish;
  end
endmodule
