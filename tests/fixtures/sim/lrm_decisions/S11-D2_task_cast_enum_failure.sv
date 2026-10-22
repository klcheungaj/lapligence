// Decision S11-D2: a task-form cast of a value that is not a member of the
// destination enumeration is a run-time error that ends the simulation at
// the call: the destination keeps its value and no later statement runs,
// but the final procedures run; the exit status is 1. The function form
// returns 0 and the run continues.
//
// IEEE 1800-2009 6.24.2 (SystemVerilog-1800-2009.txt L7430-7434): "When called
// as a task, $cast attempts to assign the source expression to the
// destination variable. If the assignment is invalid, a run-time error
// occurs, and the destination variable is left unchanged. When called as a
// function, $cast attempts to assign the source expression to the destination
// variable and returns 1 if the cast is legal. If the cast fails, the
// function does not make the assignment and returns 0." The text does not
// say whether the run-time error ends the simulation or whether final
// procedures run; llg follows industry-standard behaviour.
module tb;
  typedef enum {RED, GREEN, BLUE} color_t;
  color_t c = GREEN;
  int v = 7;
  initial begin
    $display("function: %0d, c=%0d", $cast(c, v), c);
    $cast(c, v);
    $display("after the failed cast");
    $finish;
  end
  final $display("final c=%0d", c);
endmodule
