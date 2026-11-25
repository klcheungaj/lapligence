// Static class properties through VPI: members and elements by name, through
// a class variable or the class definition, read and written (readme: "VPI").
module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } pair_t;
  typedef struct {
    int a;
    logic [7:0] f;
  } rec_t;
  typedef struct {
    string s;
    int b;
    rec_t r;
  } deep_t;
  class C;
    static pair_t pr = 8'h35;
    static rec_t rec = '{3, 8'h44};
    static deep_t de = '{"abc", 7, '{1, 2}};
    static int sa[2] = '{4, 5};
    static rec_t ra[2] = '{'{1, 2}, '{3, 4}};
    static string ss = "str";
    int x = 1;
  endclass
  C p;
  initial begin
    p = new;
    #1 $show();
    #1 $write_all();
    #1 $display("%h %0d %h %s %0d %0d %p %0d %s", C::pr, C::rec.a, C::rec.f, C::de.s, C::de.b,
                C::de.r.a, C::sa, C::ra[1].a, C::ss);
  end
  // A put to a member wakes its waiters.
  always @(C::rec.a) $display("rec.a %0d", C::rec.a);
  always @(C::pr.hi) $display("pr.hi %h", C::pr.hi);
endmodule
