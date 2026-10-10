// SIM-028 nearest illegal: srandom, get_randstate and set_randstate are
// built-in methods of every class (IEEE 1800-2009 18.13.3-18.13.5); a class
// cannot declare its own.
class packet_c;
  int id;
  function void srandom(int seed);
    id = seed;
  endfunction
endclass

module tb;
  packet_c p;
  initial begin
    p = new;
    p.srandom(1);
    $display("%0d", p.id);
    $finish;
  end
endmodule
