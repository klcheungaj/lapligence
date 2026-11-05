// Two run-time selected `ref` actuals multiply the call's branches: legal
// (SV 13.5.2) but rejected beyond the limit of one selection.
module tb;
  typedef struct { string s; } el_t;
  typedef struct { el_t e[20]; } rec_t;
  rec_t o;
  int i = 1;
  task automatic t(ref el_t a, ref el_t b);
    a.s = b.s;
  endtask
  initial t(o.e[i], o.e[i]);
endmodule
