// SIM-024: `$monitor` and `$strobe` format containers and records with the
// same pattern text; a monitor reports when a printed container changes
// (SV 21.2.2, 21.2.3).
module tb;
  typedef struct { string s; int v; } rec_t;
  int aa [int];
  rec_t r;
  initial begin
    aa[3] = 30;
    r.s = "a";
    r.v = 1;
    $monitor("mon %p %p", aa, r.s);
    #1 aa[4] = 40;
    #1 r.s = "b";
    // No monitored value changes at time 3, so the strobe prints alone.
    #1 $strobe("strobe %0p", aa);
    #1 $finish(0);
  end
endmodule
