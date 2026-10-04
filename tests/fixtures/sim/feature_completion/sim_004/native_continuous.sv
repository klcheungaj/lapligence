// SIM-004: continuous assignments to string variables and string records,
// including an output port driven by a continuous assignment.
// IEEE 1800-2009 10.3.2 (variables may be continuous targets; the value is
// updated whenever an operand changes), 23.3.3 and 6.16.
typedef struct {string s; int n;} rec_t;

module child(input string si, output string so);
  assign so = {si, "!"};
endmodule

module tb;
  string a = "old", b, d, e;
  rec_t ra, rb;
  int changes;
  child c(.si(a), .so(d));
  assign b = a;
  assign e = {b, "-", d};
  assign rb = ra;
  always_comb changes = e.len();

  initial begin
    ra = '{"r", 1};
    #1 $display("1 %s %s %s %s %0d %0d", b, d, e, rb.s, rb.n, changes);
    a = "new";
    ra.s = "q";
    #1 $display("2 %s %s %s %s %0d %0d", b, d, e, rb.s, rb.n, changes);
    a = "";
    ra = '{"z", 9};
    #1 $display("3 [%s] [%s] [%s] %s %0d %0d", b, d, e, rb.s, rb.n, changes);
    $finish(0);
  end
endmodule
