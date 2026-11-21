// Decision S22-D8: a static local whose initializer is a constant expression
// without calls (literals, parameters, enumeration constants, operators,
// casts, patterns and constant system functions such as $clog2 and $bits)
// runs with its scope's constants, before the scope's other initializers, as
// industry-standard simulators preset it: a caller before the subroutine's
// place reads the value. An initializer with a call (also a constant
// function or `new`) or that reads a variable runs at the subroutine's place
// (S22-D8_static_local_place), so such a caller reads the default value.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 6.21 L6942-6944: "an explicit static keyword shall
// be required when an initialization value is specified as part of a static
// variable's declaration to indicate the user's intent of executing that
// initialization only once at the beginning of simulation." The text does not
// order a static local against the other static initializers.
class holder;
  int v = 5;
endclass
typedef struct { int a; int b; } pair_t;
module tb;
  parameter int P = 3;
  localparam string LS = "lp";
  typedef enum int {RED = 4, BLUE = 9} color_t;
  int c1 = 5;
  int early = consts();
  string seen_str = f_str();
  int seen = kinds();
  function automatic int sq(int v);
    return v * v;
  endfunction
  function int consts();
    static int a = 1;
    static int b = a + 1;
    static int e = $bits(int);
    static int f = $signed(4'hF);
    static int g = 2 ** 4;
    static color_t h = BLUE;
    static int i = c1 + 1;
    static int k = int'(3.7);
    static int l = $clog2(256);
    static int s2 = P + 20;
    static string s3 = {LS, "y"};
    static int s4 = sq(5);
    static string s6 = $sformatf("f%0d", 6);
    static int s8[2] = '{P, 40};
    $display("consts: a=%0d b=%0d e=%0d f=%0d g=%0d h=%0d i=%0d k=%0d l=%0d", a, b, e, f, g, h, i, k, l);
    $display("consts: s2=%0d s3=%s s4=%0d s6=%s s8=%0d,%0d", s2, s3, s4, s6, s8[0], s8[1]);
    return 1;
  endfunction
  function string f_str();
    static string s = "init";
    return {"<", s, ">"};
  endfunction
  function int kinds();
    static holder hd = new;
    static int q[$] = '{1, 2, 3};
    static int d[] = '{4, 5};
    static int m[string] = '{"a": 1};
    static int arr[2] = '{8, 9};
    static pair_t pr = '{3, 4};
    static mailbox mb = new;
    $display("kinds: hd=%0d q=%0d d=%0d m=%0d arr=%0d pr=%0d mb=%0d", hd == null ? -1 : hd.v,
             q.size(), d.size(), m.num(), arr[0] + arr[1], pr.a + pr.b, mb == null ? -1 : mb.num());
    return 2;
  endfunction
  initial begin
    $display("seen_str=%s", seen_str);
    void'(consts());
    void'(kinds());
    $finish;
  end
endmodule
