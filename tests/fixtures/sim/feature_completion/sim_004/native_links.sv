// SIM-004: strings and native records cross value ports, automatic
// functions and mixed packed/native signatures as independent copies.
// IEEE 1800-2009 23.3.3 (value ports behave like continuous assignments),
// 13.4-13.5 (input formals are copies; outputs are copied out on return),
// 6.16 and 7.2.
typedef struct {string s; int n;} rec_t;

module child(input string si, output string so, input rec_t ri, output rec_t ro);
  always_comb so = {si, "!"};
  always_comb begin
    ro = ri;
    ro.n = ri.n + 1;
    ro.s = {ri.s, "+"};
  end
endmodule

module tb;
  string a = "x", b, keep, echo;
  rec_t ra, rb, rk;
  child c(.si(a), .so(b), .ri(ra), .ro(rb));

  function automatic rec_t blend(input int base, input rec_t v, input string tag,
                                 input int scale, output string text);
    v.n = v.n * scale + base;
    v.s = {v.s, tag};
    tag = "changed";
    text = {tag, ":", v.s};
    return v;
  endfunction

  task automatic stamp(input int id, input rec_t v, output rec_t w, input string tag);
    #2;
    w = v;
    w.n = w.n + id;
    w.s = {w.s, tag};
  endtask

  initial begin
    ra = '{"r", 1};
    #1 $display("1 %s %s %0d", b, rb.s, rb.n);
    a = "yz";
    ra.s = "q";
    #1 $display("2 %s %s %0d", b, rb.s, rb.n);
    keep = "t";
    rk = blend(10, ra, keep, 3, echo);
    $display("3 %s %0d %s %0d %s %s", rk.s, rk.n, ra.s, ra.n, keep, echo);
    fork
      #1 begin
        ra.s = "zz";
        keep = "late";
      end
    join_none
    stamp(5, ra, rk, keep);
    $display("4 %s %0d %s %s %s %0d", rk.s, rk.n, ra.s, keep, b, rb.n);
    $finish(0);
  end
endmodule
