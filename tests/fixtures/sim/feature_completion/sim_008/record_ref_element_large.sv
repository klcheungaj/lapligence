// A run-time selection among 257 whole records as a `ref` actual is legal
// (SV 13.5.2) but rejected: the call lowers once per element.
module tb;
  typedef struct { string s; } el_t;
  typedef struct { el_t e[257]; } rec_t;
  rec_t o;
  int i = 1;
  task automatic t(ref el_t r);
    r.s = "x";
  endtask
  initial t(o.e[i]);
endmodule
