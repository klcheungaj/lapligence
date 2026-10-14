// Decision S22-D5: a variable declared with an initializer inside a
// procedural block, without `static` or `automatic`, is accepted with a
// warning and treated as static: it is initialized once, before time 0, and
// keeps its value across loop iterations. The lines labelled `llg` show
// llg's choice; the text says such a declaration needs an explicit keyword,
// so a simulator that rejects this file conforms (see the register).
//
// IEEE 1800-2009 6.21 (SystemVerilog-1800-2009.txt L6941-6944):
//   "Variables declared in a static task, function, or procedural block
//   default to a static lifetime and a local scope. However, an explicit
//   static keyword shall be required when an initialization value is
//   specified as part of a static variable's declaration to indicate the
//   user's intent of executing that initialization only once at the
//   beginning of simulation."
// The clause's `top_illegal` example is marked "should not compile".
module tb;
  initial begin
    for (int i = 0; i < 3; i++) begin
      int count = 10;
      count++;
      $display("llg: count=%0d", count);
    end
    $finish;
  end
endmodule
