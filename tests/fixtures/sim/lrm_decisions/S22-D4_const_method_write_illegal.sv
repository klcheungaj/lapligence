// Decision S22-D4 (negative case; the expected output is empty because
// compilation fails): a built-in method that modifies the variable it is
// called on (push_back, pop_front, insert, delete, sort, reverse, shuffle,
// putc, itoa and the like) is a write, so calling it on a const variable is
// an error. Read-only methods (size, sum, len, toupper) remain legal, and a
// const class handle still lets the object's members be written.
//
// IEEE 1800-2009 6.20.6 (SystemVerilog-1800-2009.txt L6884-6886):
//   "In other words, the object acts like a variable that cannot be written.
//   The arguments to the new method shall be constant expressions (see
//   11.2.1). The members of the object can be written (except for those
//   members that are declared const)."
// The clause does not list methods; llg reads "cannot be written" as covering
// every built-in method that changes the variable's value.
module tb;
  const int limits[$] = '{3, 1, 2};
  initial begin
    $display("size=%0d sum=%0d", limits.size(), limits.sum());
    limits.push_back(4);
    $finish;
  end
endmodule
