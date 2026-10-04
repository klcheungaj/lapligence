// SIM-004 negative: nonblocking writes to automatic variables are illegal
// (IEEE 1800-2009 6.21, 10.4.2), including automatic strings.
module tb;
  task automatic t();
    string s;
    s <= "x";
  endtask
  initial begin
    t();
    $finish(0);
  end
endmodule
