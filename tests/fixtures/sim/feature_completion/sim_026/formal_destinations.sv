// SIM-026 A01: $sscanf destinations that are task and function formals
// (output, ref, inout), a selection of a ref formal and the properties of a
// method's own object (IEEE 1800-2009 21.3.4.3, 13.5).
module tb;
  class Box;
    int v;
    string s;
    function int load(string src);
      return $sscanf(src, "%d %s", v, s);
    endfunction
  endclass
  integer c, k;
  int n;
  string w;
  logic [7:0] r;
  real x;
  logic [3:0] y;
  Box box;
  task automatic scan_out(input string src, output int count, output string word);
    c = $sscanf(src, "%d %s", count, word);
  endtask
  task automatic scan_ref(input string src, ref logic [7:0] target);
    c = $sscanf(src, "%h", target[7:4]);
  endtask
  function automatic int scan_fn(input string src, inout real value, output logic [3:0] bits);
    return $sscanf(src, "%f %b", value, bits);
  endfunction
  initial begin
    box = new;
    scan_out("42 hi", n, w);
    $display("E c=%0d n=%0d w=%s", c, n, w);
    r = 8'h05;
    scan_ref("c", r);
    $display("F c=%0d r=%h", c, r);
    x = 0.0;
    k = scan_fn("0.75 101", x, y);
    $display("G k=%0d x=%f y=%b", k, x, y);
    c = box.load("9 zeta");
    $display("H c=%0d v=%0d s=%s", c, box.v, box.s);
    $finish;
  end
endmodule
