// Decision S11-D8: a class handle with no format conversion prints as `%p`
// would, so `$swrite(s, h)` gives the object's assignment pattern, or null,
// whatever the task's default radix is.
//
// IEEE 1800-2009 21.2.1 (SystemVerilog-1800-2009.txt L35884-35887): "Any
// argument expression of either a string data type or an unpacked array of
// byte data type that has no corresponding format specification shall be
// formatted as a character string. Any expression argument of any other
// unpacked data type that has no corresponding format specification shall be
// illegal." The text is silent on a class handle; llg follows
// industry-standard behaviour.
class C;
  int a = 1;
endclass
module tb;
  C c, n;
  string s;
  initial begin
    c = new;
    $swrite(s, c);
    $display("[%s]", s);
    $swrite(s, n);
    $display("[%s]", s);
    $swriteh(s, "x", n, "y");
    $display("[%s]", s);
    $display(c);
    $write(n, "\n");
  end
endmodule
