// Decision S45-D20: a coverage option value with X or Z bits becomes 0. The
// members of `option` are `int` (19.10), and a 4-state value converted to a
// 2-state one turns its unknown bits to zeros (6.11.2), so an X weight or
// at_least is 0, with no message, in the declaration and in a procedural
// write. 0 is in range for both, and the run goes on. Some simulators keep
// the option's previous value (with a warning for a procedural write); llg
// follows the clause text.
//
// IEEE 1800-2009 19.10 (SystemVerilog-1800-2009.txt L33693-33700): "The
// option and type_option members of a covergroup, coverpoint, and cross are
// implicitly declared structures with the following composition: struct //
// covergroup option declaration { string name ; int weight ; int goal ;"
// and L33702 "int at_least ;".
// IEEE 1800-2009 6.11.2 (L5489-5490): "When a 4-state value is automatically
// converted to a 2-state value, any unknown or high-impedance bits shall be
// converted to zeros."
module tb;
  bit [1:0] x;
  integer unknown = 'x;
  covergroup g(integer w);
    option.weight = w;
    option.at_least = w;
    cp: coverpoint x;
  endgroup
  g a;
  initial begin
    a = new(unknown);
    $display("declaration weight %0d at_least %0d", a.option.weight, a.option.at_least);
    a.option.weight = 3;
    a.cp.option.goal = 50;
    a.option.weight = unknown;
    a.cp.option.goal = unknown;
    $display("write weight %0d goal %0d", a.option.weight, a.cp.option.goal);
    $finish;
  end
endmodule
