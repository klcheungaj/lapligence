// Decision S22-D1: class-handle, mailbox and string declaration initializers
// of module and package variables run before any initial or always procedure
// starts, together with the other static initializers. A later static
// initializer that reads such a handle sees the constructed object, and an
// initial procedure that changes the handle at time 0 does not undo or
// repeat the construction.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5287):
//   "Setting the initial value of a static variable as part of the variable
//   declaration (including static class members) shall occur before any
//   initial or always procedures are started"
// IEEE 1800-2009 10.5 (L13286-13287) repeats the rule. Every initializer
// here reads only declarations that precede it, so declaration order and
// dependency order agree.
package pkg;
  class Counter;
    int value;
    function new(int start);
      value = start;
    endfunction
  endclass
  Counter shared = new(100);
  string label = $sformatf("shared=%0d", shared.value);
endpackage

module tb;
  class Box;
    string name;
    function new(string n);
      name = n;
    endfunction
  endclass
  Box first = new("first");
  string copied = {first.name, "!"};
  int length = first.name.len();
  mailbox #(int) box = new(4);
  int capacity_left = 4 - box.num();
  initial begin
    $display("copied=%s length=%0d capacity_left=%0d", copied, length, capacity_left);
    $display("package: %s value=%0d", pkg::label, pkg::shared.value);
    first = new("second");
    $display("after reassignment: %s copied=%s", first.name, copied);
    $finish;
  end
endmodule
