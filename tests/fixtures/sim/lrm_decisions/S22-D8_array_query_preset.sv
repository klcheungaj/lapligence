// Decision S22-D8: an initializer whose only variable operands are the first
// operands of data and array queries ($bits, $dimensions,
// $unpacked_dimensions, $left, $right, $low, $high, $increment, $size) is a
// constant, as industry-standard simulators treat it: a static local
// (j1-j9, k1-k4, k6) or a variable (e1, ma1-ma5) with such an initializer
// runs with its scope's constants, in declaration order among them. On a
// dynamic array or queue the query reads the extent the array has then: the
// extent of a constant pattern declared before it (k4, ma5), otherwise empty
// (e1 reads dd before its pattern; k2, k3 and ma3 run before the run-time
// initializers of dn2 and dq3), and the value stays. A method call such as
// `tb.dd.size()` is a call and runs at its subroutine's place (k5).
//
// IEEE 1800-2009 20.6.2 (SystemVerilog-1800-2009.txt L34635): "The value
// returned by $bits shall be determined without actual evaluation of the
// expression it encloses." 20.7 L34695-34696: "When used on a dynamic array
// or queue dimension, these functions return information about the current
// state of the array." 6.8 L5286-5288: "Setting the initial value of a static
// variable as part of the variable declaration (including static class
// members) shall occur before any initial or always procedures are started".
// The text leaves the order among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
module tb;
  import util::*;
  int z = rs();
  int e1 = $size(tb.dd);
  int arr[3];
  logic [7:0] v8;
  bit [3:0][1:0] pk;
  int dn[];
  int dn2[] = new[tick("tb.dn2")];
  int dq3[$] = '{tick("tb.dq3"), 2, 3};
  int dd[] = '{1, 2, 3, 4};
  int ma1 = $size(arr);
  int ma2 = $bits(v8) + 1;
  int ma3 = $size(dn2);
  int ma4 = $left(arr) + $right(arr) + 10;
  int ma5 = $size(dd);
  int y = rd();
  function int rs();
    static int j1 = $size(tb.arr);
    static int j2 = $bits(tb.v8);
    static int j3 = $left(tb.arr);
    static int j4 = $right(tb.pk);
    static int j5 = $high(tb.arr);
    static int j6 = $low(tb.arr);
    static int j7 = $increment(tb.arr);
    static int j8 = $dimensions(tb.pk);
    static int j9 = $unpacked_dimensions(tb.arr);
    static int k1 = $size(tb.dn);
    static int k2 = $size(tb.dn2);
    static int k3 = $size(tb.dq3);
    static int k4 = $size(tb.dd);
    static int k5 = tb.dd.size();
    static int k6 = $size(tb.arr) + $bits(tb.arr);
    $display("rs: j1=%0d j2=%0d j3=%0d j4=%0d j5=%0d j6=%0d j7=%0d j8=%0d j9=%0d",
             j1, j2, j3, j4, j5, j6, j7, j8, j9);
    $display("rs: k1=%0d k2=%0d k3=%0d k4=%0d k5=%0d k6=%0d", k1, k2, k3, k4, k5, k6);
    return 1;
  endfunction
  function int rd();
    $display("rd: e1=%0d ma1=%0d ma2=%0d ma3=%0d ma4=%0d ma5=%0d dn2=%0d dq3=%0d",
             e1, ma1, ma2, ma3, ma4, ma5, dn2.size(), dq3.size());
    return 2;
  endfunction
  initial begin
    void'(rs());
    $finish;
  end
endmodule
