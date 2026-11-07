// Three run-time selected `ref` actuals multiply the call's branches: legal
// (SV 13.5.2) but rejected beyond the limit of one selection. Elements below
// the column size are leaves, so each selection is a branch per element.
module tb;
  typedef struct { string s; } el_t;
  typedef struct { el_t e[15]; } rec_t;
  rec_t o;
  int i = 1;
  task automatic t(ref el_t a, ref el_t b, ref el_t c);
    a.s = b.s;
    b.s = c.s;
  endtask
  initial t(o.e[i], o.e[i], o.e[i]);
endmodule
