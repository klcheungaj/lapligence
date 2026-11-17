// ADV-019 typespec kinds (FIN-001 V3 family (d), 37.23): vpiPacked of
// packed and unpacked tagged unions (7.3.2); chandle typespecs (6.14):
// typedefs of chandle, chandle members, variables, class properties,
// formals and calls; void typespecs of tagged union members (6.13). typespec_kinds.c prints the typespec tree of each
// object it names and the typedefs of tb.
module tb;
  typedef union tagged packed { bit [3:0] a; bit [3:0] b; } tp_t;
  typedef union tagged { int i; real r; } tu_t;
  typedef union packed { bit [3:0] a; bit [3:0] b; } up_t;
  typedef chandle ch_t;
  typedef ch_t ch2_t;
  typedef struct { chandle h; ch_t k; } sh_t;
  typedef union tagged { chandle h; int n; } th_t;
  typedef union tagged packed { void Invalid; bit [7:0] Valid; } vp_t;
  typedef union tagged { void Invalid; int Valid; } vu_t;

  class C;
    chandle h;
    ch2_t k;
  endclass

  tp_t tp;
  tu_t tu;
  up_t up;
  chandle h;
  ch_t hn;
  ch2_t hn2;
  sh_t sh;
  th_t th;
  vp_t vp;
  vu_t vu;
  C c;
  ch_t hq[$];

  function chandle get();
    return null;
  endfunction

  task take(input chandle a, input ch_t b);
  endtask

  initial begin
    tp = tagged b 4'h5;
    tu = tagged r 1.5;
    up = 4'h3;
    th = tagged n 2;
    vp = tagged Valid 8'h5;
    vu = tagged Invalid;
    c = new;
    hq.push_back(null);
    h = get();
    take(h, hn);
    $typespec_kinds;
    $finish;
  end
endmodule
