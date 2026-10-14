// SIM-022-A03 nearest illegal (incompatible lifetimes): a static local's
// initializer reads an automatic formal.
module tb;
  task automatic keep(string text);
    static string first = text;
    $display("%s", first);
  endtask
  initial keep("x");
endmodule
