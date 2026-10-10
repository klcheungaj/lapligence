// SIM-028: set_randstate with a string that get_randstate did not produce
// is undefined (IEEE 1800-2009 18.13.5); llg reports a run-time error and
// leaves the stream unchanged (S28-D9).
class packet_c;
  int id;
endclass

module tb;
  packet_c p;
  initial begin
    p = new;
    p.set_randstate("not a random state");
    $finish;
  end
endmodule
