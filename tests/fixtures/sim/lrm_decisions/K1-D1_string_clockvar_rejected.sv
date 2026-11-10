// Decision K1-D1: a clocking input of string, class handle, event, queue,
// dynamic array or associative array type is a compile error.
//
// IEEE 1800-2009 14.3 (SystemVerilog-1800-2009.txt L19444-19447): "The
// signal_identifier specifies a signal (a net or variable) in the scope
// enclosing the clocking block declaration, and defines a clockvar in the
// clocking block. ... It shall be illegal for a clocking signal to designate
// a variable restricted to a procedural block (see 6.21)." 14.5 L19590-19592:
// "Any expression assigned to a signal in a clocking input or inout
// declaration shall be an expression that would be legal for connection to a
// module's input port." The text does not exclude these types; industry-
// standard behaviour rejects them as clocking signals, and so does llg.
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  string s = "hi";
  clocking cb @(posedge clk);
    input s;
  endclocking
  initial begin
    @(cb);
    $display("%s", cb.s);
    $finish;
  end
endmodule
