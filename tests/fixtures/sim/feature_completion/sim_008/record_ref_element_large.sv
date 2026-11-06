// A run-time selection among 257 whole records as a `ref` actual (SV 13.5.2):
// a member array of that size is one container, so the formal takes the
// selected element by reference and the call lowers once.
module tb;
  typedef struct { string s; } el_t;
  typedef struct { el_t e[257]; } rec_t;
  rec_t o;
  int i = 1;
  task automatic t(ref el_t r);
    r.s = "x";
  endtask
  initial begin
    t(o.e[i]);
    i = 256;
    t(o.e[i]);
    $display("[%s] [%s] [%s]", o.e[0].s, o.e[1].s, o.e[256].s);
    $finish;
  end
endmodule
