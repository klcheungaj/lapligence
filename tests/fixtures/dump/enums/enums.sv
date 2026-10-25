package pkg;
  typedef enum logic [1:0] {PA, PB} pe_t;
  enum {ANON0, ANON1 = 5} anon;
  typedef enum {R[2], S[1:3]} rng_t;
endpackage
module top;
  import pkg::*;
  typedef enum logic [1:0] {IDLE, RUN} state_t;
  enum {X0, X1} local_anon;
  state_t st;
  pkg::pe_t pe;
  initial begin
    st = IDLE;
    pe = pkg::PB;
    if (pe == PA) st = RUN;
    local_anon = X1;
    anon = ANON0;
  end
endmodule
