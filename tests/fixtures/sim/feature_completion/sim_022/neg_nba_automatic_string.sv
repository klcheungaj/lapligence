// SIM-022-A03 nearest illegal (incompatible lifetimes): a nonblocking
// assignment to an automatic string.
module tb;
  task automatic set();
    string text;
    text <= "x";
  endtask
  initial set();
endmodule
