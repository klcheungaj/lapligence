// Decision S24-D2: in the %p form, the assignment pattern of a non-empty
// unpacked array is followed by one space after its closing brace when
//   - the array is associative;
//   - the array is a queue or dynamic array whose elements are not unpacked
//     arrays;
//   - the array is a fixed-size array (all its unpacked dimensions together;
//     only the outermost brace is followed by the space) whose element is a
//     non-enum integral value, a real, a string or a chandle, or an unpacked
//     structure whose members are all integral (enums included), real,
//     chandle, or structures or fixed arrays of those.
// An empty array, a structure, a fixed array of enums, of class handles or
// of structures holding a string, a handle or a container, and a queue or
// dynamic array of unpacked arrays have no such space. $sformatf, $swrite,
// $write and $display print the same text.
//
// IEEE 1800-2009 21.2.1.7 (SystemVerilog-1800-2009.txt L36402-36404):
//   "The use of white space is implementation dependent; however the output
//   shall be a legal interpretation of the assignment pattern syntax (see
//   10.8)."
//
// The text leaves white space to the implementation. llg follows
// industry-standard behaviour. Values are chosen to print the same under
// either integral formatting (int elements, string keys).
module tb;
  typedef enum {ON, OFF} sw_e;
  typedef struct { int a; byte b; } pair_t;
  typedef struct { int a; sw_e e; int f[2]; } plain_t;
  typedef struct { int a; string s; } named_t;
  typedef struct { int a; int q[$]; } holder_t;
  int q[$] = '{1, 2};
  int qe[$];
  int d[] = '{5, 6, 7};
  int de[];
  int as[string];
  int ae[string];
  int u[2] = '{8, 9};
  int u2[2][3] = '{'{1, 2, 3}, '{4, 5, 6}};
  string sq[$] = '{"a", ""};
  sw_e eq[$] = '{ON, OFF};
  sw_e eu[2] = '{OFF, ON};
  pair_t p = '{3, 4};
  pair_t pu[2] = '{'{1, 2}, '{3, 4}};
  plain_t lu[2];
  named_t nu[2];
  holder_t h;
  holder_t hu[2];
  int qq[$][$];
  int qf[$][2];
  int aq[string][$];
  string text;
  initial begin
    as["x"] = 1; as["yy"] = 2;
    qq.push_back(q); qq.push_back(qe);
    qf.push_back(u);
    aq["k"] = q; aq["m"] = qe;
    h.a = 1; h.q.push_back(5);
    hu[0] = h;
    $display("q=[%p] qe=[%p]", q, qe);
    $display("d=[%p] de=[%p]", d, de);
    $display("as=[%p] ae=[%p]", as, ae);
    $display("u=[%p] u2=[%p]", u, u2);
    $display("sq=[%p] eq=[%p] eu=[%p]", sq, eq, eu);
    $display("p=[%p] pu=[%p]", p, pu);
    $display("lu=[%p]", lu);
    $display("nu=[%p]", nu);
    $display("h=[%p] hu=[%p]", h, hu);
    $display("qq=[%p] qf=[%p]", qq, qf);
    $display("aq=[%p]", aq);
    $write("w=[%p]\n", q);
    text = $sformatf("[%p]", q);
    $display("sformatf %s %0d", text, text.len());
    $swrite(text, "[%p]", qe);
    $display("swrite %s %0d", text, text.len());
    $finish;
  end
endmodule
