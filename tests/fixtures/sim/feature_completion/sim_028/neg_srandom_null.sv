// SIM-028: a built-in random method through a null handle is a null object
// access, a run-time error (IEEE 1800-2009 8.4).
class packet_c;
  int id;
endclass

module tb;
  packet_c p;
  initial begin
    p.srandom(3);
    $finish;
  end
endmodule
